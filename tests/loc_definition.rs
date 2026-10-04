// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The `loc` definition, graded against an independent reference.
//!
//! Every JSON object on the loc wire that carries numeric `start` / `end` — in all three
//! languages, objects without a `type` included — carries `loc: {start: {line, column},
//! end: {line, column}}` immediately after `end`: the line (1-based) and column (0-based,
//! UTF-16 code units) of that object's own `start` / `end`. One line-terminator rule per
//! DOCUMENT: ECMAScript's (CR, LF, CRLF, LS, PS) for a TypeScript document, LF alone for a
//! Svelte document — everything embedded in it included — and for a CSS document. A leading
//! byte-order mark is counted by the TypeScript wire and elided by the Svelte and CSS ones,
//! as each canonical parser does. No other object carries `loc`, and a `character` field,
//! where a position has one, is that position's own offset.
//!
//! A Svelte name's `name_loc` is graded the same way: each of its points is the line and
//! column of its own `character`, and the span those name is the name itself — an element's
//! from just past its `<`, an attribute's from its start (past the `{` of a shorthand), each
//! as wide as the `name` it carries; a directive's from its start to the end of its head
//! (`on:click|once`), which Svelte's reader stops at whitespace or one of `=` `/` `>` `"`
//! `'`.
//!
//! And the span-only wire — the one every binding ships — is the loc wire with every
//! `loc` / `name_loc` removed and nothing else, byte for byte (`span_wire_difference`).
//! The grader lives in `support/loc_wire.rs`, shared with the tests whose inputs no fixture
//! can hold.
//!
//! The reference here is the test's own — its own line-start scan over the document's
//! UTF-16 units — so it shares nothing with `tsv_lang`'s line table. It walks every fixture
//! input in the tree, then the inputs no fixture can hold: a line terminator other than LF
//! (the format path folds CR, and no fixture input is anything but its own formatted
//! output), a byte-order mark, an astral character.

use serde_json::Value;
use std::path::Path;
use tsv_debug::fixtures::{self, InputType};

#[path = "support/utf16_lines.rs"]
mod utf16_lines;

#[path = "support/loc_wire.rs"]
mod loc_wire;
use loc_wire::{assert_definition, span_wire_difference, violations, wires_and_reference};

/// `loc` of the first object (pre-order) whose `type` is `node_type` and whose source
/// slice — by UTF-16 offsets — is `text`, as `(start line, start column, end line, end
/// column)`.
fn loc_of(wire: &Value, node_type: &str, units: &[u16], text: &str) -> (u64, u64, u64, u64) {
    fn find<'a>(value: &'a Value, node_type: &str, units: &[u16], text: &str) -> Option<&'a Value> {
        match value {
            Value::Object(map) => {
                if map.get("type").and_then(Value::as_str) == Some(node_type)
                    && let (Some(start), Some(end)) = (
                        map.get("start").and_then(Value::as_u64),
                        map.get("end").and_then(Value::as_u64),
                    )
                    && String::from_utf16_lossy(&units[start as usize..end as usize]) == text
                {
                    return Some(value);
                }
                map.values().find_map(|v| find(v, node_type, units, text))
            }
            Value::Array(items) => items.iter().find_map(|v| find(v, node_type, units, text)),
            _ => None,
        }
    }
    let node = find(wire, node_type, units, text).expect("a node of that type spells that text");
    let loc = &node["loc"];
    let n = |side: &str, field: &str| loc[side][field].as_u64().expect("a loc number");
    (
        n("start", "line"),
        n("start", "column"),
        n("end", "line"),
        n("end", "column"),
    )
}

fn units(source: &str) -> Vec<u16> {
    source.encode_utf16().collect()
}

#[test]
fn every_fixture_input_follows_the_definition() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    // per input kind: Svelte, Svelte TS module, TypeScript, CSS
    let mut graded = [0usize; 4];
    let mut failures = Vec::new();
    for fixture in fixtures::walk_fixtures(&root).expect("walk the fixture tree") {
        // tsv rejects the input on purpose; there is no wire to grade.
        if fixture.tsv_rejects_path().exists() {
            continue;
        }
        let input_type = fixture.input_type();
        let source = fixtures::read_file(&fixture.input_path()).expect("read the input");
        let (mut wire, span, reference) = wires_and_reference(&source, input_type, fixture.goal());
        let mut found = violations(&wire, &reference);
        found.extend(span_wire_difference(&mut wire, &span));
        if !found.is_empty() {
            failures.push(format!(
                "{}:\n  {}",
                fixture.relative_path,
                found
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n  ")
            ));
        }
        graded[match input_type {
            InputType::Svelte => 0,
            InputType::SvelteTs => 1,
            InputType::TypeScript => 2,
            InputType::Css => 3,
        }] += 1;
    }
    assert!(
        failures.is_empty(),
        "{} fixture input(s) break the loc definition:\n{}",
        failures.len(),
        failures
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Every input kind reached the grade: a walk that silently stopped finding one would
    // leave its language ungraded.
    assert!(
        graded.iter().all(|&n| n > 0),
        "input kinds graded: {graded:?}"
    );
}

#[test]
fn a_leading_bom_is_counted_by_typescript_and_elided_by_svelte_and_css() {
    // acorn counts the BOM: the statement sits at offset 1, column 1.
    let source = "\u{feff}a;\nb;";
    let wire = assert_definition(source, InputType::TypeScript);
    assert_eq!(
        loc_of(&wire, "Identifier", &units(source), "a"),
        (1, 1, 1, 2)
    );
    // Svelte and `parseCss` strip it: every offset indexes the BOM-less string.
    let source = "\u{feff}<p>{a}</p>";
    let wire = assert_definition(source, InputType::Svelte);
    assert_eq!(
        loc_of(&wire, "RegularElement", &units(&source[3..]), "<p>{a}</p>"),
        (1, 0, 1, 10)
    );
    let source = "\u{feff}p { color: red; }";
    let wire = assert_definition(source, InputType::Css);
    assert_eq!(
        loc_of(&wire, "Rule", &units(&source[3..]), "p { color: red; }"),
        (1, 0, 1, 17)
    );
}

/// A Svelte document counts LF alone — in its `<script>`, its template expressions and
/// its `<style>` too — so a lone CR, U+2028 or U+2029 anywhere in it opens no line.
#[test]
fn a_svelte_document_counts_lf_alone_in_every_island() {
    let source = concat!(
        "<script>\n",
        "let a = 1;\rlet b = 2;\u{2028}let c = 3;\u{2029}let d = 4;\n",
        "</script>\n",
        "<p>{x\r+\u{2028}y}</p>\n",
        "<style>\n",
        "p { color: red; }\r/* \u{2028} */ a { color: blue; }\n",
        "</style>\n",
    );
    let wire = assert_definition(source, InputType::Svelte);
    let units = units(source);
    assert_eq!(loc_of(&wire, "Identifier", &units, "d"), (2, 37, 2, 38));
    assert_eq!(loc_of(&wire, "Identifier", &units, "y"), (4, 8, 4, 9));
    assert_eq!(
        loc_of(&wire, "Rule", &units, "a { color: blue; }"),
        (6, 26, 6, 44)
    );
}

/// The same rule inside the islands Svelte's readers hand acorn a manufactured string for —
/// a block binding and its `: T`, a `{@const}` declarator, a bare declaration tag, a
/// `{#snippet}` head: a lone CR in any of them opens no line, so what follows it on the
/// document line, the island's own nodes included, keeps that line.
#[test]
fn a_lone_cr_inside_a_template_reader_island_opens_no_line() {
    let source = concat!(
        "<script lang=\"ts\"></script>\n",
        "{#each xs as {a}\r: T}{a}{/each}\n",
        "{#if c}{@const {b} = c\r}{b}{/if}\n",
        "{const d = 1\r}\n",
        "{#snippet s(e\r)}{e}{/snippet}\n",
    );
    let wire = assert_definition(source, InputType::Svelte);
    let units = units(source);
    assert_eq!(
        loc_of(&wire, "TSTypeReference", &units, "T"),
        (2, 19, 2, 20)
    );
    assert_eq!(
        loc_of(&wire, "ExpressionTag", &units, "{b}"),
        (3, 24, 3, 27)
    );
    assert_eq!(
        loc_of(&wire, "DeclarationTag", &units, "{const d = 1\r}"),
        (4, 0, 4, 14)
    );
    assert_eq!(
        loc_of(&wire, "ExpressionTag", &units, "{e}"),
        (5, 16, 5, 19)
    );
}

/// A TypeScript document counts ECMAScript's terminators, so the same statements open
/// lines at each.
#[test]
fn a_typescript_document_counts_every_ecmascript_terminator() {
    let source = "let a = 1;\rlet b = 2;\u{2028}let c = 3;\u{2029}let d = 4;\r\nlet e = 5;";
    let wire = assert_definition(source, InputType::TypeScript);
    let units = units(source);
    assert_eq!(loc_of(&wire, "Identifier", &units, "b"), (2, 4, 2, 5));
    assert_eq!(loc_of(&wire, "Identifier", &units, "c"), (3, 4, 3, 5));
    assert_eq!(loc_of(&wire, "Identifier", &units, "d"), (4, 4, 4, 5));
    // CRLF is one terminator.
    assert_eq!(loc_of(&wire, "Identifier", &units, "e"), (5, 4, 5, 5));
}

/// Columns count UTF-16 code units: an astral character ahead of a node is two.
#[test]
fn columns_count_utf16_units() {
    let source = "/*😀*/ a;";
    let wire = assert_definition(source, InputType::TypeScript);
    assert_eq!(
        loc_of(&wire, "Identifier", &units(source), "a"),
        (1, 7, 1, 8)
    );
    let source = "<p>😀{a}</p>";
    let wire = assert_definition(source, InputType::Svelte);
    assert_eq!(
        loc_of(&wire, "ExpressionTag", &units(source), "{a}"),
        (1, 5, 1, 8)
    );
    let source = "p::after { content: '😀'; } a { }";
    let wire = assert_definition(source, InputType::Css);
    assert_eq!(
        loc_of(&wire, "Rule", &units(source), "a { }"),
        (1, 28, 1, 33)
    );
}

#[test]
fn crlf_is_one_line_break_in_every_language() {
    let source = "a;\r\nb;";
    let wire = assert_definition(source, InputType::TypeScript);
    assert_eq!(
        loc_of(&wire, "Identifier", &units(source), "b"),
        (2, 0, 2, 1)
    );
    let source = "<p>\r\n{a}</p>";
    let wire = assert_definition(source, InputType::Svelte);
    assert_eq!(
        loc_of(&wire, "ExpressionTag", &units(source), "{a}"),
        (2, 0, 2, 3)
    );
    let source = "p { }\r\na { }";
    let wire = assert_definition(source, InputType::Css);
    assert_eq!(loc_of(&wire, "Rule", &units(source), "a { }"), (2, 0, 2, 5));
}

/// The `<script>` `Program`'s `loc` is its content span's, like every other node's — an
/// indented tag included.
#[test]
fn a_script_program_loc_follows_its_content_span() {
    let source = "<div></div>\t<script>\n  let a = 1;\n</script>";
    let wire = assert_definition(source, InputType::Svelte);
    let program = &wire["instance"]["content"];
    assert_eq!(program["start"], 20);
    assert_eq!(
        (
            &program["loc"]["start"]["line"],
            &program["loc"]["start"]["column"]
        ),
        (&Value::from(1), &Value::from(20))
    );
    assert_eq!(
        (
            &program["loc"]["end"]["line"],
            &program["loc"]["end"]["column"]
        ),
        (&Value::from(3), &Value::from(0))
    );
}

/// The shapes Svelte gives a `loc` of its own, where tsv's follows the definition instead:
/// a destructured block binding off line 1, its typed form whose `end` is widened to the
/// annotation, and a newline in the four units before a binding's colon.
#[test]
fn block_bindings_follow_the_definition() {
    for source in [
        "\n{#each xs as { a, b }}{a}{/each}",
        "\n{#each xs as { a }: T}{a}{/each}",
        "\n{#each xs as [a, /* c */ b]}{a}{/each}",
        "{#each xs as x\n: T}{x}{/each}",
        "\n{#await p then { v }}{v}{:catch [e]}{e}{/await}",
        "\n{@const { a } = b}",
    ] {
        assert_definition(source, InputType::Svelte);
    }
}

/// The objects with `start` / `end` and no `type` carry `loc` too.
#[test]
fn objects_without_a_type_carry_loc() {
    let source = concat!(
        "<svelte:options runes />\n",
        "<p class=\"a {b}\" {c}>{/* d */ e}</p>\n",
        "<style>\n/* f */\np { }\n</style>\n",
    );
    let wire = assert_definition(source, InputType::Svelte);
    assert!(wire["options"]["loc"].is_object(), "SvelteOptions");
    assert!(
        wire["css"]["content"]["loc"].is_object(),
        "StyleSheet.content"
    );
    assert!(wire["comments"][0]["loc"].is_object(), "a root comment");
}

/// Every shape that carries a `name_loc` — elements, plain and shorthand attributes, each
/// directive kind with and without modifiers — names exactly its name, on a line past the
/// first, behind a multi-unit character.
#[test]
fn name_locs_name_their_names() {
    let source = concat!(
        "\u{feff}<p>😀</p>\n",
        "<svelte:element this={t} {x} bind:value on:click|once={f} class:a style:b=\"c\"\n",
        "\tuse:act transition:fade|local animate:flip let:item in:fly out:fly a=b c></svelte:element>\n",
        "<Comp\u{a0}on:é={g}/><svelte:head></svelte:head>",
    );
    let wire = assert_definition(source, InputType::Svelte);
    fn count(value: &Value) -> usize {
        match value {
            Value::Object(map) => {
                usize::from(map.contains_key("name_loc")) + map.values().map(count).sum::<usize>()
            }
            Value::Array(items) => items.iter().map(count).sum(),
            _ => 0,
        }
    }
    // four elements, three attributes, eleven directives
    assert_eq!(
        count(&wire),
        18,
        "every name in the document carries a name_loc"
    );
}
