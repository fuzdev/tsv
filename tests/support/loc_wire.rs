//! The `loc` wire grader the wire-coordinate tests share: the `loc` definition checked
//! against the independent line reference (`support/utf16_lines.rs`), and the span-only
//! wire checked to be the loc wire stripped. Included by `#[path]` beside
//! `support/utf16_lines.rs`, which it reads as `super::utf16_lines`: by `loc_definition.rs`,
//! whose fixture walk and synthetic inputs it grades, and by
//! `comment_dedent_document_line.rs`, whose `<script>`-reader rows no fixture can hold.

use super::utf16_lines::{LineRule, line_column, line_starts, wire_rule, wire_text};
use serde_json::Value;
use tsv_debug::fixtures::{self, InputType};

/// A document as its wire positions index it: its UTF-16 code units (behind the BOM when
/// the wire elides one) and the unit offset each line starts at.
pub struct Reference {
    units: Vec<u16>,
    line_starts: Vec<usize>,
}

impl Reference {
    fn new(source: &str, rule: LineRule, elide_bom: bool) -> Self {
        let units: Vec<u16> = wire_text(source, elide_bom).encode_utf16().collect();
        let line_starts = line_starts(&units, rule);
        Self { units, line_starts }
    }

    /// `(line, column)` of a UTF-16 offset.
    fn position(&self, offset: usize) -> (u64, u64) {
        let (line, column) = line_column(&self.line_starts, offset);
        (line as u64, column as u64)
    }
}

/// Every violation of the definition in `value`, each named by its JSON path.
pub fn violations(value: &Value, reference: &Reference) -> Vec<String> {
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
    // a directive's span is its whole head, so only the other two spell their `name`
    let directive = node_type.ends_with("Directive");
    let (want_from, want_to, what) = if node_type == "Attribute" {
        // a shorthand `{x}` names its identifier; a static `<script>` attribute may be
        // named `{a}` itself
        let open =
            usize::from(units.get(start) == Some(&u16::from(b'{')) && !name.starts_with('{'));
        (start + open, start + open + width, "an attribute's name")
    } else if !directive {
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
    if !directive
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
pub fn wires_and_reference(
    source: &str,
    input_type: InputType,
    goal: tsv_ts::Goal,
) -> (Value, Vec<u8>, Reference) {
    let arena = bumpalo::Bump::new();
    let (loc, span, rule, elide_bom) = match input_type {
        InputType::Svelte => {
            let ast = tsv_svelte::parse(source, &arena).expect("Svelte input parses");
            let (rule, elide_bom) = wire_rule(false);
            (
                tsv_svelte::convert_ast_json_bytes_with_locations(&ast, source),
                tsv_svelte::convert_ast_json_bytes(&ast, source),
                rule,
                elide_bom,
            )
        }
        InputType::TypeScript | InputType::SvelteTs => {
            let ast = tsv_ts::parse_with_goal(source, goal, &arena).expect("TS input parses");
            let (rule, elide_bom) = wire_rule(true);
            (
                tsv_ts::convert_ast_json_bytes_with_locations(&ast, source),
                tsv_ts::convert_ast_json_bytes(&ast, source),
                rule,
                elide_bom,
            )
        }
        InputType::Css => {
            let ast = tsv_css::parse(source, &arena).expect("CSS input parses");
            let (rule, elide_bom) = wire_rule(false);
            (
                tsv_css::convert_ast_json_bytes_with_locations(&ast, source),
                tsv_css::convert_ast_json_bytes(&ast, source),
                rule,
                elide_bom,
            )
        }
    };
    (
        tsv_debug::json::wire_value(&loc),
        span,
        Reference::new(source, rule, elide_bom),
    )
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
pub fn span_wire_difference(loc_wire: &mut Value, span: &[u8]) -> Option<String> {
    fixtures::strip_locations(loc_wire);
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
pub fn assert_definition(source: &str, input_type: InputType) -> Value {
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
