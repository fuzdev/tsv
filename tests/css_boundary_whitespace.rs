// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used, clippy::panic)]

//! CSS boundary whitespace is **JS `\s`**, not the CSS Syntax class and not Rust's.
//!
//! Svelte's `parseCss` reads a stylesheet with two rules that disagree about the same code
//! points, and resolves the disagreement by ORDER
//! (`svelte/packages/svelte/src/compiler/phases/1-parse/read/style.js`):
//!
//! - `parser.allow_whitespace()` — the template parser's class, which is exactly JS `\s`
//!   (`is_whitespace` in `1-parse/index.js`; the same set `tsv_svelte`'s `is_svelte_ws`
//!   enumerates). It includes `<NBSP>`, every `Zs`, `<LS>`, `<PS>` and `<ZWNBSP>`, and it
//!   **excludes** `<NEL>` (U+0085), which is `Cc` rather than `Zs` — ECMA-262 leaves it out
//!   deliberately.
//! - `read_identifier` — accepts `[a-zA-Z0-9_-]`, escapes, and **any code point ≥ U+00A0**.
//!
//! So a code point in both sets is whitespace when an `allow_whitespace()` reaches it first
//! and identifier content when `read_identifier` does. The junctures decide: Svelte skips
//! whitespace at a selector-list start, after a `,`, inside a `[`, after a combinator and
//! after a `;`, and skips **nothing** between a sigil (`.` `#` `:` `::`) and the name it
//! introduces, where selectors-4 forbids whitespace anyway.
//!
//! tsv reads ONE class at every boundary — `char::is_whitespace()` below U+00A0 — which is
//! wrong in both directions: it misses every JS-`\s` code point at or above U+00A0 (folding
//! it into the following name) and it over-matches `<NEL>` (accepting input Svelte rejects).
//! This test is the pin for both.
//!
//! **Why a test rather than a fixture.** None of these inputs is a formatting fixed point:
//! once the character is whitespace the formatter normalizes it away, so it cannot be an
//! `input.*` (F1), and prettier's CSS keeps it inside the selector, so it cannot be an
//! `unformatted_*` variant either (both formatters would have to normalize to `input`).
//! Same arrangement, and the same weakness, as `tests/css_cdo_cdc.rs`: every expectation
//! here is transcribed from the live modern parser
//! (`cargo run -p tsv_debug canonical_parse`) rather than regenerated, so it would go stale
//! silently if `parseCss` changed.

//! **Residue, deliberately not closed here.** The junctures above are the ones the wire
//! audit surfaced and this fix covers; four more still read the run as identifier content,
//! all of them span- or structure-level rather than name-level, and none reachable from
//! authored CSS:
//!
//! - a descendant combinator's `end` when the run trails the space (`a <NBSP>b`);
//! - the compound break after a complete simple selector that is not an identifier — `&<NBSP>b`
//!   and `*<NBSP>b`, where canonical starts a new compound and tsv folds the run into the next
//!   name. The one **name-level** member, and the one that cannot be fixed by moving a
//!   capture: the chain loop has to break AND the combinator that replaces the break has to
//!   materialize, and doing only the first turns both spellings into parse errors (measured —
//!   see the note at that loop in `parser/selectors.rs`);
//! - a pseudo-class argument list's own `start` (`:is(<NBSP>b)`);
//! - a `<ZWNBSP>` leading a declaration VALUE, which `read_value` drops and tsv keeps —
//!   the mirror image of the value fixture this class deliberately leaves alone.

use serde_json::Value;

/// Every named CSS node in the wire, in emission order: selectors by `name`, declarations by
/// `property`. Enough to say which side of a boundary a character landed on.
fn named_nodes(src: &str) -> Vec<(String, String)> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let mut out = Vec::new();
    collect(json.get("css").unwrap_or(&Value::Null), &mut out);
    out
}

fn collect(node: &Value, out: &mut Vec<(String, String)>) {
    match node {
        Value::Object(fields) => {
            let ty = fields.get("type").and_then(Value::as_str).unwrap_or("");
            let key = if ty == "Declaration" {
                "property"
            } else {
                "name"
            };
            if (ty == "Declaration" || ty.ends_with("Selector"))
                && let Some(name) = fields.get(key).and_then(Value::as_str)
            {
                out.push((ty.to_owned(), name.to_owned()));
            }
            for value in fields.values() {
                collect(value, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect(i, out)),
        _ => {}
    }
}

/// Whether tsv accepts the component at all.
fn parses(src: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_svelte::parse(src, &arena).is_ok()
}

fn component(style: &str) -> String {
    format!("<div>x</div>\n\n{style}\n")
}

/// The JS-`\s` members at or above U+00A0 — the half tsv currently folds into the following
/// identifier. `<ZWNBSP>` is in the set although it is `Cf` rather than `White_Space`, which
/// is exactly why the class cannot be `char::is_whitespace()`.
const JS_WHITESPACE_AT_OR_ABOVE_A0: [(&str, &str); 5] = [
    ("NBSP", "\u{a0}"),
    ("LS", "\u{2028}"),
    ("PS", "\u{2029}"),
    ("IDEOGRAPHIC SPACE", "\u{3000}"),
    ("ZWNBSP", "\u{feff}"),
];

/// `<NEL>`, the one `White_Space` code point JS `\s` excludes — and so the one tsv currently
/// treats as whitespace where Svelte does not.
const NEL: &str = "\u{85}";

/// The five junctures where Svelte runs `allow_whitespace()` before the next token starts.
/// `{T}` marks the injection point; the pair is the node the character must NOT reach.
const BOUNDARY_JUNCTURES: [(&str, &str, (&str, &str)); 5] = [
    (
        "selector-list start",
        "<style>{T}div { color: red; }</style>",
        ("TypeSelector", "div"),
    ),
    (
        "after a comma",
        "<style>a,{T}div { color: red; }</style>",
        ("TypeSelector", "div"),
    ),
    (
        "inside an attribute selector",
        "<style>[{T}a] { color: red; }</style>",
        ("AttributeSelector", "a"),
    ),
    (
        "after a combinator",
        "<style>a >{T}b { color: red; }</style>",
        ("TypeSelector", "b"),
    ),
    (
        "after a declaration's semicolon",
        "<style>a { color: red;{T}top: 0; }</style>",
        ("Declaration", "top"),
    ),
];

/// The two junctures where `read_identifier` reaches the character first, so it is identifier
/// CONTENT. These pass today and are the null controls the fix must not break — a widening
/// applied at the lexer's every token start would turn the first into a parse error, since
/// selectors-4 forbids whitespace between a sigil and its name and tsv rejects `. b`.
const IDENT_CONTENT_JUNCTURES: [(&str, &str, &str); 2] = [
    (
        "after a `.` sigil",
        "<style>.{T}b { color: red; }</style>",
        "ClassSelector",
    ),
    (
        "inside an identifier",
        "<style>.a{T}b { color: red; }</style>",
        "ClassSelector",
    ),
];

#[test]
fn a_js_whitespace_code_point_is_whitespace_at_every_allow_whitespace_juncture() {
    for (juncture, template, (expected_type, expected_name)) in BOUNDARY_JUNCTURES {
        for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
            let src = component(&template.replace("{T}", ch));
            let named = named_nodes(&src);
            assert!(
                named
                    .iter()
                    .any(|(ty, name)| ty == expected_type && name == expected_name),
                "{label} {juncture}: expected a {expected_type} named {expected_name:?}, \
                 wire held {named:?}"
            );
        }
    }
}

#[test]
fn a_js_whitespace_code_point_is_identifier_content_where_no_skip_precedes_it() {
    for (juncture, template, expected_type) in IDENT_CONTENT_JUNCTURES {
        for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
            let src = component(&template.replace("{T}", ch));
            let named = named_nodes(&src);
            assert!(
                named
                    .iter()
                    .any(|(ty, name)| ty == expected_type && name.contains(ch)),
                "{label} {juncture}: the character belongs to the name, wire held {named:?}"
            );
        }
    }
}

/// ⚠️ **RATCHET, not a claim of correctness.** `<NEL>` (U+0085) is the one code point Rust's
/// `char::is_whitespace()` calls whitespace and JS `\s` does not, so `parseCss` neither skips
/// it nor lets `read_identifier` take it (it is below U+00A0) — it **rejects** wherever a name
/// is read, and keeps it as **content** in the two positions it scans raw, a declaration's
/// property and value. tsv's CSS lexer still reads it as whitespace in every position, so it
/// over-accepts the first and silently drops it in the second.
///
/// It is deliberately left alone by the boundary class above, because it is not a member of
/// that family: the raw-scan positions are a **separate, pre-existing** gap, where every
/// character the raw scan accepts and an identifier token does not already diverges — `%top`,
/// `!top` and `(top` reach the wire silently truncated to `top`, and `1top` / `+top` are
/// rejected outright. Dropping `<NEL>` from the lexer's class in isolation would fix the
/// selector half and turn the property half from a silent drop into a **new over-rejection**,
/// which is the wrong trade to make ahead of the raw readers. Both halves are pinned here so
/// closing that family has to come back and re-pin them together. See
/// [conformance_svelte.md §CSS Parser Scope & Error Model](../docs/conformance_svelte.md).
#[test]
fn next_line_is_a_tracked_gap_in_both_directions() {
    // Over-acceptance: canonical rejects every one of these (`read_identifier` refuses a
    // code point below U+00A0), and tsv's lexer reads the character as whitespace and carries
    // on. The `.` / `#` / `:` sigils are NOT in this set — there tsv rejects too, because a
    // whitespace token after a sigil is already an error, so the two agree by accident.
    for (juncture, template) in BOUNDARY_JUNCTURES
        .iter()
        .filter(|(_, _, (ty, _))| *ty != "Declaration")
        .map(|(juncture, template, _)| (*juncture, *template))
        .chain(std::iter::once((
            "inside an identifier",
            "<style>.a{T}b { color: red; }</style>",
        )))
    {
        assert!(
            parses(&component(&template.replace("{T}", NEL))),
            "NEL {juncture}: canonical REJECTS this and tsv accepts it — if that changed, \
             re-pin this ratchet and the catalog entry it names"
        );
    }

    // …and where the sigil path already refuses a whitespace token, the two agree.
    for (juncture, template, _) in IDENT_CONTENT_JUNCTURES {
        if template.contains("{T}b {") && template.contains(".{T}") {
            assert!(
                !parses(&component(&template.replace("{T}", NEL))),
                "NEL {juncture}: both sides reject this today"
            );
        }
    }

    // Silent drop: canonical scans a declaration's property raw, so it keeps the character.
    let named = named_nodes(&component("<style>a { color: red;\u{85}top: 0; }</style>"));
    assert!(
        named
            .iter()
            .any(|(ty, name)| ty == "Declaration" && name == "top"),
        "NEL before a property: canonical keeps it as part of the name and tsv drops it — if that \
         changed, re-pin this ratchet; wire held {named:?}"
    );
}

/// ⚠️ **RATCHET for the residue.** Four junctures still read a boundary run as identifier
/// content. They are enumerated in this module's doc; asserted here so "documented but
/// unchecked" cannot quietly become "documented and wrong" — a prose-only gap is exactly the
/// kind that outlives the code it describes.
///
/// Each is span- or structure-level rather than name-level, which is why the fix above landed
/// without them: the selector NAMES already agree at every juncture.
#[test]
fn the_remaining_junctures_are_a_tracked_gap() {
    // A descendant combinator's `end` stops before the run; canonical's covers it.
    let combinator_end = |src: &str| {
        let arena = bumpalo::Bump::new();
        let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
        let json = tsv_svelte::convert_ast_json(&ast, src);
        let mut found = Vec::new();
        fn walk(node: &Value, out: &mut Vec<u64>) {
            match node {
                Value::Object(fields) => {
                    if fields.get("type").and_then(Value::as_str) == Some("Combinator") {
                        out.extend(fields.get("end").and_then(Value::as_u64));
                    }
                    fields.values().for_each(|v| walk(v, out));
                }
                Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
                _ => {}
            }
        }
        walk(&json, &mut found);
        found
    };
    assert_eq!(
        combinator_end(&component("<style>a \u{a0}b { color: red; }</style>")),
        vec![23],
        "the descendant combinator stops before the run; canonical ends it at 24"
    );

    // `&<NBSP>b` and `*<NBSP>b` are one compound to tsv and two relative selectors to
    // canonical, and the run ends up inside the following NAME. Both spellings are pinned
    // because they fail identically and a fix has to move them together.
    for style in [
        "<style>a { &\u{a0}b { color: red; } }</style>",
        "<style>*\u{a0}b { color: red; }</style>",
    ] {
        let named = named_nodes(&component(style));
        assert!(
            named
                .iter()
                .any(|(ty, name)| ty == "TypeSelector" && name == "\u{a0}b"),
            "the run is still folded into the name here — wire held {named:?}"
        );
    }

    // A `<ZWNBSP>` leading a declaration VALUE: `read_value` drops it, tsv keeps it. The
    // mirror image of the value fixture this class deliberately leaves alone, and the reason
    // the boundary skip stops at the colon.
    // (`named_nodes` reports a Declaration by `property`; this one needs its `value`, so it
    // reads the field directly rather than through the shared collector.)
    let arena = bumpalo::Bump::new();
    let src = component("<style>a { color:\u{feff}red; }</style>");
    let ast = tsv_svelte::parse(&src, &arena).expect("component should parse");
    let json = tsv_svelte::convert_ast_json(&ast, &src);
    let mut values = Vec::new();
    fn walk_values(node: &Value, out: &mut Vec<String>) {
        match node {
            Value::Object(fields) => {
                if fields.get("type").and_then(Value::as_str) == Some("Declaration")
                    && let Some(v) = fields.get("value").and_then(Value::as_str)
                {
                    out.push(v.to_owned());
                }
                fields.values().for_each(|v| walk_values(v, out));
            }
            Value::Array(items) => items.iter().for_each(|i| walk_values(i, out)),
            _ => {}
        }
    }
    walk_values(&json, &mut values);
    assert_eq!(
        values,
        vec!["\u{feff}red".to_owned()],
        "tsv keeps the ZWNBSP in the value; canonical's `read_value` drops it (`'red'`)"
    );
}
