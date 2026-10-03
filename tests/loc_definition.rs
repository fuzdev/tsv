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
//!
//! The reference here is the test's own — its own line-start scan over the document's
//! UTF-16 units — so it shares nothing with `tsv_lang`'s line table. It walks every fixture
//! input in the tree, then the inputs no fixture can hold: a line terminator other than LF
//! (the format path folds CR, and no fixture input is anything but its own formatted
//! output), a byte-order mark, an astral character.

use serde_json::Value;
use std::path::Path;
use tsv_debug::fixtures::{self, InputType};

/// Which characters end a line in a document.
#[derive(Clone, Copy, Debug)]
enum LineRule {
    /// LF, CR, CRLF (one terminator), U+2028, U+2029 — a TypeScript document.
    Ecmascript,
    /// LF alone — a Svelte document and everything in it, and a CSS document.
    Lf,
}

/// A document as its wire positions index it: its UTF-16 code units (behind the BOM when
/// the wire elides one) and the unit offset each line starts at.
struct Reference {
    units: Vec<u16>,
    line_starts: Vec<usize>,
}

impl Reference {
    fn new(source: &str, rule: LineRule, elide_bom: bool) -> Self {
        let document = if elide_bom {
            source.strip_prefix('\u{feff}').unwrap_or(source)
        } else {
            source
        };
        let units: Vec<u16> = document.encode_utf16().collect();
        let mut line_starts = vec![0];
        for (i, &unit) in units.iter().enumerate() {
            let ends_line = match rule {
                LineRule::Lf => unit == 0x0a,
                LineRule::Ecmascript => match unit {
                    0x0a | 0x2028 | 0x2029 => true,
                    // A CR followed by its LF is one terminator, ended by the LF.
                    0x0d => units.get(i + 1) != Some(&0x0a),
                    _ => false,
                },
            };
            if ends_line {
                line_starts.push(i + 1);
            }
        }
        Self { units, line_starts }
    }

    /// `(line, column)` of a UTF-16 offset: the line is how many lines start at or
    /// before it.
    fn position(&self, offset: usize) -> (u64, u64) {
        let line = self.line_starts.partition_point(|&start| start <= offset);
        (line as u64, (offset - self.line_starts[line - 1]) as u64)
    }
}

/// Every violation of the definition in `value`, each named by its JSON path.
fn violations(value: &Value, reference: &Reference) -> Vec<String> {
    let mut out = Vec::new();
    walk(value, reference, &mut String::from("$"), &mut out);
    out
}

fn walk(value: &Value, reference: &Reference, path: &mut String, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            let offsets = map
                .get("start")
                .and_then(Value::as_u64)
                .zip(map.get("end").and_then(Value::as_u64));
            match offsets {
                Some((start, end)) => check_loc(map, start, end, reference, path, out),
                None => {
                    if map.contains_key("loc") {
                        out.push(format!(
                            "{path}: `loc` on an object with no numeric start/end"
                        ));
                    }
                }
            }
            if map.contains_key("name_loc") {
                check_name_loc(map, reference, path, out);
            }
            for (key, child) in map {
                let len = path.len();
                path.push('.');
                path.push_str(key);
                walk(child, reference, path, out);
                path.truncate(len);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let len = path.len();
                path.push('[');
                path.push_str(&i.to_string());
                path.push(']');
                walk(item, reference, path, out);
                path.truncate(len);
            }
        }
        _ => {}
    }
}

fn check_loc(
    map: &serde_json::Map<String, Value>,
    start: u64,
    end: u64,
    reference: &Reference,
    path: &str,
    out: &mut Vec<String>,
) {
    let keys: Vec<&str> = map.keys().map(String::as_str).collect();
    let after_end = keys
        .iter()
        .position(|&k| k == "end")
        .and_then(|i| keys.get(i + 1));
    if after_end != Some(&"loc") {
        out.push(format!(
            "{path}: `loc` is not the key after `end` (keys {keys:?})"
        ));
        return;
    }
    let loc = &map["loc"];
    let Some(loc_map) = loc.as_object() else {
        out.push(format!("{path}: `loc` is not an object"));
        return;
    };
    let loc_keys: Vec<&str> = loc_map.keys().map(String::as_str).collect();
    if loc_keys != ["start", "end"] {
        out.push(format!("{path}.loc: keys {loc_keys:?}"));
    }
    for (side, offset) in [("start", start), ("end", end)] {
        let point = &loc[side];
        let (line, column) = reference.position(offset as usize);
        let want = (Some(line), Some(column));
        let got = (point["line"].as_u64(), point["column"].as_u64());
        if got != want {
            out.push(format!(
                "{path}.loc.{side}: offset {offset} is {want:?} by the definition, wire has {got:?}"
            ));
        }
        if let Some(character) = point.get("character")
            && character.as_u64() != Some(offset)
        {
            out.push(format!(
                "{path}.loc.{side}.character: {character} is not the offset {offset}"
            ));
        }
        let point_keys: Vec<&str> = point
            .as_object()
            .map(|p| p.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if point_keys != ["line", "column"] && point_keys != ["line", "column", "character"] {
            out.push(format!("{path}.loc.{side}: keys {point_keys:?}"));
        }
    }
}

/// Whether a UTF-16 unit is in JavaScript's `\s` class — the whitespace Svelte's reader
/// stops a name at (Rust's own class differs: it holds U+0085 and lacks U+FEFF).
fn is_js_whitespace(unit: u16) -> bool {
    matches!(
        unit,
        0x09..=0x0d
            | 0x20
            | 0xa0
            | 0x1680
            | 0x2000..=0x200a
            | 0x2028
            | 0x2029
            | 0x202f
            | 0x205f
            | 0x3000
            | 0xfeff
    )
}

fn check_name_loc(
    map: &serde_json::Map<String, Value>,
    reference: &Reference,
    path: &str,
    out: &mut Vec<String>,
) {
    let Some(name_loc) = map["name_loc"].as_object() else {
        out.push(format!("{path}.name_loc: not an object"));
        return;
    };
    let keys: Vec<&str> = name_loc.keys().map(String::as_str).collect();
    if keys != ["start", "end"] {
        out.push(format!("{path}.name_loc: keys {keys:?}"));
        return;
    }
    let mut characters = [0usize; 2];
    for (i, side) in ["start", "end"].into_iter().enumerate() {
        let point = &name_loc[side];
        let point_keys: Vec<&str> = point
            .as_object()
            .map(|p| p.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if point_keys != ["line", "column", "character"] {
            out.push(format!("{path}.name_loc.{side}: keys {point_keys:?}"));
            return;
        }
        let Some(character) = point["character"].as_u64() else {
            out.push(format!("{path}.name_loc.{side}.character: not a number"));
            return;
        };
        let (line, column) = reference.position(character as usize);
        let got = (point["line"].as_u64(), point["column"].as_u64());
        if got != (Some(line), Some(column)) {
            out.push(format!(
                "{path}.name_loc.{side}: character {character} is {:?} by the definition, wire has {got:?}",
                (line, column)
            ));
        }
        characters[i] = character as usize;
    }
    let [from, to] = characters;
    let (Some(start), Some(name)) = (
        map.get("start").and_then(Value::as_u64),
        map.get("name").and_then(Value::as_str),
    ) else {
        out.push(format!(
            "{path}: `name_loc` without a numeric start and a name"
        ));
        return;
    };
    let start = start as usize;
    let units = &reference.units;
    let width = name.encode_utf16().count();
    let node_type = map.get("type").and_then(Value::as_str).unwrap_or_default();
    let (want_from, want_to, what) = if node_type == "Attribute" {
        // a shorthand `{x}` names its identifier; a static `<script>` attribute may be
        // named `{a}` itself
        let open =
            usize::from(units.get(start) == Some(&u16::from(b'{')) && !name.starts_with('{'));
        (start + open, start + open + width, "an attribute's name")
    } else if !node_type.ends_with("Directive") {
        (start + 1, start + 1 + width, "an element's name")
    } else {
        let head_end = (start..units.len())
            .find(|&i| {
                is_js_whitespace(units[i]) || b"=/>\"'".iter().any(|&b| units[i] == u16::from(b))
            })
            .unwrap_or(units.len());
        let head = String::from_utf16_lossy(&units[start..head_end]);
        let names_it = head
            .split_once(':')
            .is_some_and(|(_, rest)| rest.starts_with(name));
        if !names_it {
            out.push(format!(
                "{path}.name_loc: the directive head {head:?} does not name {name:?}"
            ));
        }
        (start, head_end, "a directive's head")
    };
    if what != "a directive's head"
        && units.get(want_from..want_to) != Some(&name.encode_utf16().collect::<Vec<_>>()[..])
    {
        out.push(format!(
            "{path}: {what} {name:?} is not the source at [{want_from}, {want_to})"
        ));
    }
    if (from, to) != (want_from, want_to) {
        out.push(format!(
            "{path}.name_loc: spans [{from}, {to}), {what} is [{want_from}, {want_to})"
        ));
    }
}

/// Both wires of `source` parsed as `input_type` — the loc wire as a tree, the span-only
/// wire as its bytes — with the loc wire's reference.
fn wires_and_reference(
    source: &str,
    input_type: InputType,
    goal: tsv_ts::Goal,
) -> (Value, Vec<u8>, Reference) {
    let arena = bumpalo::Bump::new();
    let (loc, span, rule, elide_bom) = match input_type {
        InputType::Svelte => {
            let ast = tsv_svelte::parse(source, &arena).expect("Svelte input parses");
            (
                tsv_svelte::convert_ast_json_bytes_with_locations(&ast, source),
                tsv_svelte::convert_ast_json_bytes(&ast, source),
                LineRule::Lf,
                true,
            )
        }
        InputType::TypeScript | InputType::SvelteTs => {
            let ast = tsv_ts::parse_with_goal(source, goal, &arena).expect("TS input parses");
            (
                tsv_ts::convert_ast_json_bytes_with_locations(&ast, source),
                tsv_ts::convert_ast_json_bytes(&ast, source),
                LineRule::Ecmascript,
                false,
            )
        }
        InputType::Css => {
            let ast = tsv_css::parse(source, &arena).expect("CSS input parses");
            (
                tsv_css::convert_ast_json_bytes_with_locations(&ast, source),
                tsv_css::convert_ast_json_bytes(&ast, source),
                LineRule::Lf,
                true,
            )
        }
    };
    (
        tsv_debug::json::wire_value(&loc),
        span,
        Reference::new(source, rule, elide_bom),
    )
}

/// Remove every `loc` and `name_loc` key — the exact set the span-only wire omits
/// (`character` lives inside both). `shift_remove`, not `remove`: under `preserve_order`
/// the latter is a `swap_remove`, which moves the object's last key into the hole.
fn strip_locations(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.shift_remove("loc");
            map.shift_remove("name_loc");
            for child in map.values_mut() {
                strip_locations(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_locations(item);
            }
        }
        _ => {}
    }
}

/// The span-only wire must be the loc wire with every `loc` / `name_loc` removed and
/// nothing else, byte for byte: so the two wires agree on every `type` / `start` / `end`
/// and payload, in the same key order, and the fixtures' grade of the span-only wire
/// against the canonical parsers carries over to the loc wire. Bytes, not `Value`s —
/// `serde_json`'s `Map` equality ignores key order even under `preserve_order` — and the
/// re-serialization is exact (`preserve_order` keeps the writer's key order,
/// `arbitrary_precision` each number token, and the writer's string escaper is graded
/// byte-for-byte against `serde_json`'s). Strips `loc_wire` in place; returns where the
/// two first differ.
fn span_wire_difference(loc_wire: &mut Value, span: &[u8]) -> Option<String> {
    strip_locations(loc_wire);
    let stripped = serde_json::to_string(loc_wire).expect("a parsed wire re-serializes");
    let at = stripped
        .bytes()
        .zip(span)
        .position(|(a, &b)| a != b)
        .or_else(|| (stripped.len() != span.len()).then(|| stripped.len().min(span.len())))?;
    let context = |text: &[u8]| {
        String::from_utf8_lossy(&text[at.saturating_sub(40)..(at + 40).min(text.len())])
            .into_owned()
    };
    Some(format!(
        "span-only wire != the loc wire stripped, at byte {at}:\n    stripped  …{}…\n    span-only …{}…",
        context(stripped.as_bytes()),
        context(span)
    ))
}

/// Grade one document, panicking with the first violations.
fn assert_definition(source: &str, input_type: InputType) -> Value {
    let (wire, span, reference) = wires_and_reference(source, input_type, tsv_ts::Goal::Module);
    let mut found = violations(&wire, &reference);
    found.extend(span_wire_difference(&mut wire.clone(), &span));
    assert!(
        found.is_empty(),
        "{input_type:?} {source:?} breaks the loc definition:\n{}",
        found
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    wire
}

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
        graded[input_type as usize] += 1;
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
