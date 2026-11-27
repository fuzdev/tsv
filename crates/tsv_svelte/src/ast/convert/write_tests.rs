//! The wire writer's unit tests, over its JSON copy. They sit beside the mounts rather than
//! inside `write.rs`, which the tape copy mounts a second time: a test module there would be
//! compiled into both copies, and its JSON-wire assertions mean nothing in the tape's.
//!
//! The tests that read `name_loc` / `loc` need `tsv_lang`'s `_locations` feature — on in every
//! workspace-wide test run (`tsv_cli` and `tsv_debug` enable it), off in a bare
//! `cargo test -p tsv_svelte` — and are gated on it one by one; the rest grade the span-only
//! wire and run either way.

use serde_json::Value;

/// Parse full Svelte source and return its span-only wire.
fn convert_svelte(source: &str) -> Value {
    // Test inputs are hardcoded valid sources; a parse failure should panic
    let root = crate::__private::parse(source).expect("parse");
    serde_json::from_slice(&crate::__private::convert_ast_json_bytes(&root, source)).expect("wire")
}

/// Parse full Svelte source and return its loc-bearing wire.
#[cfg(feature = "locations")]
fn convert_svelte_with_locations(source: &str) -> Value {
    let root = crate::__private::parse(source).expect("parse");
    serde_json::from_slice(&crate::__private::convert_ast_json_bytes_with_locations(
        &root, source,
    ))
    .expect("wire")
}

// Svelte hard-codes a `{@const}` declaration's `end` to `parser.index - 1`
// (the byte before the closing `}`) — independent of interior whitespace
// and of whether the document carries template comments. Not expressible
// as a fixture: the trigger (whitespace before `}`) is never format-stable.
#[test]
fn const_tag_declaration_end_is_byte_before_closing_brace() {
    // `}` at byte 28; the init ends at 27 — the end must be 28.
    let ast = convert_svelte("{#snippet s()}{@const x = 1 }{/snippet}");
    let decl = &ast["fragment"]["nodes"][0]["body"]["nodes"][0]["declaration"];
    assert_eq!(decl["end"], 28);

    // The same tag in a comment-bearing document: identical end.
    let ast = convert_svelte("{#snippet s()}{@const x = 1 }{/snippet}\n{/* c */ y}");
    let decl = &ast["fragment"]["nodes"][0]["body"]["nodes"][0]["declaration"];
    assert_eq!(decl["end"], 28);
}

/// Every node's `name_loc` from the wire, as `(start, end)` `{line, column, character}`.
#[cfg(feature = "locations")]
fn name_locs(node: &Value, out: &mut Vec<(Value, Value)>) {
    match node {
        Value::Object(fields) => {
            if let Some(loc) = fields.get("name_loc") {
                out.push((loc["start"].clone(), loc["end"].clone()));
            }
            fields.values().for_each(|v| name_locs(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| name_locs(v, out)),
        _ => {}
    }
}

/// `write_name_loc_field` resolves only a name's start to a line and derives the end
/// from it — sound because no name span holds a `\n`, the one byte Svelte's line table
/// opens a line at. Graded here over every name shape that carries a `name_loc`
/// (element, component, special element, attribute, directive head with modifiers),
/// each written directly before and after every line-terminator spelling of either
/// class — `\n`, `\r`, `\r\n`, `<LS>`, `<PS>` — and after a multibyte name or a
/// multibyte line ahead of it: each endpoint must sit at the line and column an
/// independent count over the source's UTF-16 units gives its `character` (a line
/// opens after each `\n`), and each end on its start's line. The oracle is the test's
/// own, so the grade holds without debug assertions; the writer's debug assertion also
/// re-derives both endpoints the unfused way on every name the test suite parses.
#[test]
#[cfg(feature = "locations")]
fn name_loc_end_shares_its_start_line() {
    for term in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}", "\t", " "] {
        for lead in ["", "é\n", "😀", "\r\n\n"] {
            let sources = [
                format!("{lead}<div{term}class=\"a\"{term}id={term}b></div>"),
                format!("{lead}<Foo.Bar{term}x{term}/>"),
                format!("{lead}<svelte:head{term}></svelte:head>"),
                format!("{lead}<a{term}on:click|once|preventDefault={{f}}{term}>é</a>"),
                format!("{lead}<p{term}bind:value={{v}}{term}class:é={{c}}></p>"),
                format!("{lead}<input{term}data-é{term}/>"),
                format!("{lead}<div{term}{{x}}{term}{{é}}></div>"),
            ];
            for source in &sources {
                let units: Vec<u16> = source.encode_utf16().collect();
                let mut locs = Vec::new();
                name_locs(&convert_svelte_with_locations(source), &mut locs);
                assert!(!locs.is_empty(), "{source:?} carries a name_loc");
                for (start, end) in locs {
                    assert_eq!(start["line"], end["line"], "{source:?}");
                    for point in [&start, &end] {
                        let character = point["character"].as_u64().expect("character") as usize;
                        let before = &units[..character];
                        let line_start = before
                            .iter()
                            .rposition(|&u| u == u16::from(b'\n'))
                            .map_or(0, |i| i + 1);
                        let line = 1 + before.iter().filter(|&&u| u == u16::from(b'\n')).count();
                        assert_eq!(point["line"], line, "{source:?} {point}");
                        assert_eq!(
                            point["column"],
                            character - line_start,
                            "{source:?} {point}"
                        );
                    }
                }
            }
        }
    }
}

/// One element holding each shape the element and attribute writers tell apart.
const SHAPES: &str = concat!(
    r#"<div a b="c" d="{e}" f={g} {h} i="j{k}">t{x}<!--c-->"#,
    r#"<textarea>y{z}</textarea><textarea></textarea><Foo /></div>"#,
);

/// The element and attribute writers fold the constants around their dynamic parts —
/// an empty attribute list, a boolean attribute's `true`, a lone text's array, each
/// header's positions — so the bytes are pinned whole, for every such shape at once:
/// a boolean, lone-text, quoted-tag, bare-tag, shorthand and mixed attribute; a text,
/// a tag and a comment child; a `<textarea>` with a sequence text and an empty one;
/// a childless component.
#[test]
fn element_and_attribute_shapes_write_these_bytes() {
    let root = crate::__private::parse(SHAPES).expect("parse");
    let wire = crate::__private::convert_ast_json_bytes(&root, SHAPES);
    let expected = concat!(
        r#"{"css":null,"js":[],"start":0,"end":111,"type":"Root","fragment":{"type":"Fragment","#,
        r#""nodes":[{"type":"RegularElement","start":0,"end":111,"name":"div","#,
        r#""attributes":[{"type":"Attribute","start":5,"end":6,"name":"a","value":true},"#,
        r#"{"type":"Attribute","start":7,"end":12,"name":"b","value":[{"type":"Text","start":10,"#,
        r#""end":11,"raw":"c","data":"c"}]},"#,
        r#"{"type":"Attribute","start":13,"end":20,"name":"d","value":[{"type":"ExpressionTag","#,
        r#""start":16,"end":19,"expression":{"type":"Identifier","start":17,"end":18,"#,
        r#""name":"e"}}]},"#,
        r#"{"type":"Attribute","start":21,"end":26,"name":"f","value":{"type":"ExpressionTag","#,
        r#""start":23,"end":26,"expression":{"type":"Identifier","start":24,"end":25,"name":"g"}}},"#,
        r#"{"type":"Attribute","start":27,"end":30,"name":"h","value":{"type":"ExpressionTag","#,
        r#""start":28,"end":29,"expression":{"type":"Identifier","start":28,"end":29,"name":"h"}}},"#,
        r#"{"type":"Attribute","start":31,"end":39,"name":"i","value":[{"type":"Text","start":34,"#,
        r#""end":35,"raw":"j","data":"j"},"#,
        r#"{"type":"ExpressionTag","start":35,"end":38,"expression":{"type":"Identifier","#,
        r#""start":36,"end":37,"name":"k"}}]}],"fragment":{"type":"Fragment","#,
        r#""nodes":[{"type":"Text","start":40,"end":41,"raw":"t","data":"t"},"#,
        r#"{"type":"ExpressionTag","start":41,"end":44,"expression":{"type":"Identifier","#,
        r#""start":42,"end":43,"name":"x"}},"#,
        r#"{"type":"Comment","start":44,"end":52,"data":"c"},"#,
        r#"{"type":"RegularElement","start":52,"end":77,"name":"textarea","attributes":[],"#,
        r#""fragment":{"type":"Fragment","nodes":[{"type":"Text","start":62,"end":63,"raw":"y","#,
        r#""data":"y"},"#,
        r#"{"type":"ExpressionTag","start":63,"end":66,"expression":{"type":"Identifier","#,
        r#""start":64,"end":65,"name":"z"}}]}},"#,
        r#"{"type":"RegularElement","start":77,"end":98,"name":"textarea","attributes":[],"#,
        r#""fragment":{"type":"Fragment","nodes":[]}},"#,
        r#"{"type":"Component","start":98,"end":105,"name":"Foo","attributes":[],"#,
        r#""fragment":{"type":"Fragment","nodes":[]}}]}}]},"options":null,"comments":[]}"#
    );
    assert_eq!(String::from_utf8(wire).expect("UTF-8 wire"), expected);
}

/// `node` with every position moved `by` code units along its line: `start`, `end`,
/// `character` and `column`, wherever one is a number.
fn shifted(node: &Value, by: u64) -> Value {
    match node {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| {
                    let moved = match (key.as_str(), value.as_u64()) {
                        ("start" | "end" | "character" | "column", Some(n)) => Value::from(n + by),
                        _ => shifted(value, by),
                    };
                    (key.clone(), moved)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(|v| shifted(v, by)).collect()),
        other => other.clone(),
    }
}

/// Every header writes its `start` / `end` into a fixed-width window, so each is
/// walked across every decimal width a document reaches: the same element behind a
/// one-line lead of every length that puts one of its offsets on a power of ten must
/// be the unpadded element with its positions moved by the lead — in both wire
/// variants. (A value past eight digits takes the writer's own wide arm, graded with
/// `JsonWriter::start_end_head`.)
#[test]
fn positions_survive_every_digit_width() {
    let wires = |source: &str| {
        let root = crate::__private::parse(source).expect("parse");
        [
            #[cfg(feature = "locations")]
            crate::__private::convert_ast_json_bytes_with_locations(&root, source),
            crate::__private::convert_ast_json_bytes(&root, source),
        ]
        .map(|wire| serde_json::from_slice::<Value>(&wire).expect("wire"))
    };
    let bare = wires(SHAPES);
    let lead_shell = "<i></i>".len();
    for power in [10usize, 100, 1_000, 10_000, 100_000] {
        let widest = power + 1;
        let narrowest = power.saturating_sub(SHAPES.len() + 1).max(lead_shell);
        for lead in narrowest..=widest {
            let source = format!("<i>{}</i>{SHAPES}", "p".repeat(lead - lead_shell));
            for (padded, bare) in wires(&source).iter().zip(&bare) {
                assert_eq!(
                    padded["fragment"]["nodes"][1],
                    shifted(&bare["fragment"]["nodes"][0], lead as u64),
                    "lead {lead}"
                );
            }
        }
    }
}

/// `write_name_loc_field` writes the end's line as the start's digits again: graded
/// at each decimal width of the line, over every name shape, against the line the
/// source puts the name on.
#[test]
#[cfg(feature = "locations")]
fn name_loc_line_repeats_at_every_digit_width() {
    for line in [
        1usize, 9, 10, 99, 100, 999, 1_000, 9_999, 10_000, 99_999, 100_000,
    ] {
        let source = format!(
            "{}<div class=\"a\" {{b}} on:click|once={{f}}><Foo.Bar /></div><svelte:head></svelte:head>",
            "\n".repeat(line - 1)
        );
        let mut locs = Vec::new();
        name_locs(&convert_svelte_with_locations(&source), &mut locs);
        assert_eq!(locs.len(), 6, "line {line}");
        for (start, end) in locs {
            assert_eq!(start["line"], line, "line {line}");
            assert_eq!(end["line"], line, "line {line}");
        }
    }
}

/// `write_raw_then_data` hands `JsonWriter::string_pair_whitespace` every text whose
/// `is_collapsible_ws_only` flag is set, and that window's escape table restates the
/// flag's class, `[ \t\n\r]`, because `tsv_lang` cannot see this crate's
/// [`is_collapsible_ws`](crate::ast::is_collapsible_ws). Graded here, where
/// both are in reach: every byte the class admits must write byte-identical to
/// `serde_json` — a lone byte and a run on the window's arm, a run on its long arm, each
/// into a full buffer and one with room — so a widened class fails this test rather
/// than writing an unescaped control byte into a release build's wire.
#[test]
fn whitespace_window_covers_the_collapsible_class() {
    for byte in (0..=u8::MAX).filter(|&b| crate::ast::is_collapsible_ws(b)) {
        for len in [1, 16, 17] {
            let text = String::from_utf8(vec![byte; len]).expect("an ASCII run");
            let quoted = serde_json::to_string(&text).expect("serde_json serializes a str");
            let expected = format!("{quoted},\"data\":{quoted}");
            for cap in [0, 256] {
                let mut w = tsv_lang::JsonWriter::with_capacity(cap);
                w.string_pair_whitespace(text.as_bytes(), b",\"data\":");
                assert_eq!(
                    String::from_utf8(w.into_bytes()).expect("UTF-8 wire"),
                    expected,
                    "{byte:#04x} x {len} (capacity {cap})"
                );
            }
        }
    }
}

// A `{let}`/`{const}` DeclarationTag keeps acorn's declaration `end`
// (canonical Svelte applies no `-1` rewrite there, unlike `{@const}`) — in
// both document states.
#[test]
fn declaration_tag_end_is_acorns_declaration_end() {
    let ast = convert_svelte("{let x = 1 }");
    assert_eq!(ast["fragment"]["nodes"][0]["declaration"]["end"], 10);

    let ast = convert_svelte("{let x = 1 }\n{/* c */ y}");
    assert_eq!(ast["fragment"]["nodes"][0]["declaration"]["end"], 10);
}
