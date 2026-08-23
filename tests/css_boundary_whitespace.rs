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
//! whitespace at a selector-list start, after a `,`, inside a `[`, after a combinator, after
//! a `;`, and at each child of an at-rule block, and skips **nothing** between a sigil
//! (`.` `#` `:` `::`) and the name it introduces, where selectors-4 forbids whitespace anyway.
//!
//! tsv resolves it the same way and in the same order: the lexer reads a code point at or
//! above U+00A0 as identifier content, and `CssParser::skip_boundary_whitespace` steps the run
//! back off at each of the junctures above. One direction is still open — the lexer's own
//! class is `char::is_whitespace()`, which over-matches `<NEL>` (U+0085) and so accepts input
//! Svelte rejects. This test pins the closed half and ratchets the open one.
//!
//! ⚠️ **A boundary run is ONE run however its members are spelled.** `<NBSP><SP>` is a single
//! `allow_whitespace()` to `parseCss`, and only the non-ASCII half is hiding inside an
//! identifier token — so the parser's skip loops rather than stepping once, and the printer's
//! preservation scans back over both classes rather than the non-ASCII one alone. Getting
//! either wrong is invisible to the single-character spellings above: the leftover ASCII gap
//! is swallowed by the next ordinary whitespace skip, leaving the NAMES right and moving only
//! the offsets captured before it — or, at `[` and after a combinator, leaving a gap where a
//! name was due and turning input canonical accepts into a parse error.
//!
//! **Why a test rather than a fixture.** None of these inputs is a formatting fixed point:
//! once the character is whitespace the formatter normalizes it away, so it cannot be an
//! `input.*` (F1), and prettier's CSS keeps it inside the selector, so it cannot be an
//! `unformatted_*` variant either (both formatters would have to normalize to `input`).
//! Same arrangement, and the same weakness, as `tests/css_cdo_cdc.rs`: every expectation
//! here is transcribed from the live modern parser
//! (`cargo run -p tsv_debug canonical_parse`) rather than regenerated, so it would go stale
//! silently if `parseCss` changed.
//!
//! **Residue, deliberately not closed here.** The junctures above are the ones the wire
//! audit surfaced and this fix covers; three more still read the run as identifier content,
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
//! - a pseudo-class argument list's own `start` (`:is(<NBSP>b)`).
//!
//! A fourth — a `<ZWNBSP>` leading or trailing a declaration VALUE — is CLOSED, and its
//! assertion moved down into the parity test: the wire's own trims are JS `\s` now
//! (`ast/convert/mod.rs`'s `trim_wire*`, mirroring `read_value`'s `value.trim()`), where
//! they used to be `str::trim` — which kept a `<ZWNBSP>` `read_value` drops and deleted a
//! `<NEL>` it keeps.

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

/// The junctures where Svelte runs `allow_whitespace()` before the next token starts.
/// `{T}` marks the injection point; the pair is the node the character must NOT reach.
const BOUNDARY_JUNCTURES: [(&str, &str, (&str, &str)); 8] = [
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
    // The at-rule block is the one `allow_comment_or_whitespace` juncture that does not
    // route through `skip_html_comment_markers` — legacy `<!--` markers are a
    // stylesheet-body and style-rule-block form — so both of its arms are pinned: the
    // rule-list one a conditional group takes, and the declaration one CSS block-parsing
    // agnosticism allows in the same place.
    (
        "an at-rule block's first rule",
        "<style>@media screen {{T}div { color: red } }</style>",
        ("TypeSelector", "div"),
    ),
    (
        "an at-rule block's first declaration",
        "<style>@media screen {{T}color: red }</style>",
        ("Declaration", "color"),
    ),
    (
        "a nested at-rule block's first declaration",
        "<style>a { @media screen {{T}color: red } }</style>",
        ("Declaration", "color"),
    ),
];

/// The junctures where a run is followed by a COMMENT, whose disposition belongs to the
/// juncture rather than to the skip.
///
/// Their own family because the assertion is different: a comment is not a named node, and
/// what goes wrong is not a name but a DROP or a rejection. The boundary skip stops on a
/// comment exactly where the plain whitespace skip does — which is what leaves the loop's
/// own comment arm in charge — and stepping the run *after* that arm instead put the cursor
/// on a comment already passed, where the "skip unexpected token" tail ate it silently.
const COMMENT_AFTER_RUN_JUNCTURES: [(&str, &str); 4] = [
    (
        "a stylesheet body",
        "<style>{T}/* c */ div { color: red; }</style>",
    ),
    (
        "a style rule's block",
        "<style>a { b {} {T}/* c */ d { color: blue; } }</style>",
    ),
    (
        "before a block's closing brace",
        "<style>a { color: red; {T}/* c */ }</style>",
    ),
    (
        "an at-rule block",
        "<style>@media screen {{T}/* c */ div { color: red } }</style>",
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

    // A hex escape's OPTIONAL TERMINATOR is `read_identifier`'s `(\r\n|\s)?` — a JS regex, so
    // `<NEL>` does not fill it and the identifier ends at the escape. Canonical REJECTS the
    // whole thing (the `<NEL>` that follows is not an identifier code point either), so this
    // is the same over-acceptance as above and pinned the same way; what it adds is that the
    // SHAPE is now the one the right class produces. Asked here because it is the one place
    // the terminator's class is observable on its own: the wire's half-decode
    // (`raw_selector_name`) is the second reader of that same rule, and the `<ZWNBSP>` witness
    // that grades BOTH readers together is a fixture
    // (`svelte/style/escape_terminator_unicode_space`), which this shape cannot be.
    let named = named_nodes(&component("<style>.a\\41\u{85}b { color: red; }</style>"));
    assert!(
        named.iter().any(|(_, name)| name == "aA"),
        "NEL after a hex escape must not fill the terminator slot: the name ends at the escape \
         (`aA`), and the character reaches the lexer's own whitespace class as the separate \
         gap this test pins. Absorbed instead, it reads `aA<NEL>b` — one name, one class \
         wrong; wire held {named:?}"
    );
}

/// ⚠️ **RATCHET for the residue.** Three junctures still read a boundary run as identifier
/// content. They are enumerated in this module's doc, and **all three** are asserted here so
/// "documented but unchecked" cannot quietly become "documented and wrong" — a prose-only gap
/// is exactly the kind that outlives the code it describes, and the count in this sentence is
/// the one thing a reader checks it against, so a juncture closed without an assertion removed
/// leaves the doc claiming a gap that is gone.
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

    // A pseudo-class argument list's own `start`: tsv opens the inner list ON the run,
    // canonical one character past it. The third juncture, and the one the two assertions
    // above cannot reach — the inner selector's NAME is `b` on both sides, and the outer
    // list's `start` is the rule's, untouched. Its `end` agrees too, so only the opening
    // offset moves.
    let src = component("<style>:is(\u{a0}b) { color: red; }</style>");
    let run_start = utf16_offset_of(&src, "\u{a0}b");
    assert_eq!(
        selector_list_starts(&src),
        vec![utf16_offset_of(&src, ":is("), run_start],
        "the argument list still opens ON the run; canonical opens it at {} — wire held {:?}",
        run_start + 1,
        selector_list_starts(&src)
    );
}

/// A declaration's own boundary trims are `read_value`'s `value.trim()` / `value.trimStart()`
/// and `read_declaration`'s `\s`-terminated property read — JS `\s`, so each drops a
/// `<ZWNBSP>` and keeps a `<NEL>`. Both directions, since `str::trim` (which these used to be)
/// gets each one wrong the other way: it kept the `<ZWNBSP>` and deleted the `<NEL>`, so a
/// single-witness test would have graded a half-fix as done.
///
/// **Every arm, because they are one rule reached six ways.** `strip_css_comments_inner` owns
/// the trim, and three shortcuts stand in for it where nothing needs stripping — the
/// no-`/*` fast path, `split_declaration_svelte_compat`'s two arms, and `write_declaration`'s
/// no-comment value. A shortcut on a different class is the shortcut silently disagreeing
/// with what it shortcuts, so each is driven by an input that reaches only it: a bare
/// declaration, one whose comment sits in the property gap, and one whose comment sits in
/// the value.
///
/// Wire claims, deliberately, not format ones. The `<NEL>` half never reaches the printer —
/// the value's own token ends at it in the lexer, which is the separate raw-scan gap
/// `next_line_is_a_tracked_gap_in_both_directions` pins.
#[test]
fn a_declarations_boundary_trims_are_the_js_class() {
    let value_of = |style: &str| {
        let arena = bumpalo::Bump::new();
        let src = component(style);
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
        values
    };

    // `<ZWNBSP>` is JS `\s`, so the trim takes it off either end — once per arm.
    //
    // A comment in the PROPERTY gap makes the declaration comment-bearing without putting a
    // `/*` in the value, which is the only way to reach the strip's no-`/*` fast path; a
    // comment in the VALUE reaches the owned path's own two trims. The `value` a
    // property-gap comment produces carries the comment and the colon, per the Svelte quirk
    // `split_declaration_svelte_compat` reproduces — so the claim there is the trailing end.
    for (style, expected) in [
        ("<style>a { color:\u{feff}red; }</style>", "red"),
        ("<style>a { color: red\u{feff}; }</style>", "red"),
        ("<style>a { color /* c */: red\u{feff}; }</style>", ": red"),
        (
            "<style>a { color:\u{feff}/* c */red\u{feff}; }</style>",
            "red",
        ),
        // A comment ahead of the PROPERTY leaves the quirk's `before_comment` empty, so the
        // split falls to its normal arm and hands the strip a value with no `/*` in it —
        // the one input that reaches the strip's no-comment fast path.
        ("<style>a { /* c */color: red\u{feff}; }</style>", "red"),
        // …and one GLUED to the property reaches the quirk arm's own trim, which decides
        // where the property name ends.
        ("<style>a { color\u{feff}/* c */: red; }</style>", ": red"),
    ] {
        assert_eq!(
            value_of(style),
            vec![expected.to_owned()],
            "ZWNBSP: {style}"
        );
    }

    // `<NEL>` is not, so it stays — the null control a `char::is_whitespace` trim fails.
    assert_eq!(
        value_of("<style>a { color: red\u{85}; }</style>"),
        vec!["red\u{85}".to_owned()],
        "NEL is not JS `\\s`, so `read_value` keeps it in the value"
    );

    // The PROPERTY side is the same rule read by `read_declaration`, which stops the name at
    // the first `\s` and then `allow_whitespace()`s — so a `<ZWNBSP>` never reaches the name
    // and a `<NEL>` is part of it.
    let property_of = |style: &str| {
        named_nodes(&component(style))
            .into_iter()
            .filter(|(ty, _)| ty == "Declaration")
            .map(|(_, name)| name)
            .collect::<Vec<_>>()
    };
    for style in [
        "<style>a { color\u{feff}: red; }</style>",
        // The quirk arm's own property trim, same claim one path over.
        "<style>a { color\u{feff}/* c */: red; }</style>",
    ] {
        assert_eq!(
            property_of(style),
            vec!["color".to_owned()],
            "ZWNBSP is `\\s`, so it is not part of the property name: {style}"
        );
    }
    assert_eq!(
        property_of("<style>a { color\u{85}: red; }</style>"),
        vec!["color\u{85}".to_owned()],
        "NEL is not `\\s`, so it IS part of the property name"
    );
}

/// UTF-16 offset of the first occurrence of `needle` — the units the wire counts, which are
/// not byte offsets on any source this file builds.
fn utf16_offset_of(src: &str, needle: &str) -> u64 {
    let byte = src.find(needle).expect("needle should occur in the source");
    src[..byte].encode_utf16().count() as u64
}

/// Every `SelectorList` `start` in the wire, in emission order — a rule's own list first, then
/// any a pseudo-class argument opens. The offset a rule, its prelude and its first complex
/// selector all share, and the one a name-level assertion cannot see.
fn selector_list_starts(src: &str) -> Vec<u64> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let mut found = Vec::new();
    fn walk(node: &Value, out: &mut Vec<u64>) {
        match node {
            Value::Object(fields) => {
                if fields.get("type").and_then(Value::as_str) == Some("SelectorList") {
                    out.extend(fields.get("start").and_then(Value::as_u64));
                }
                fields.values().for_each(|v| walk(v, out));
            }
            Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
            _ => {}
        }
    }
    walk(&json, &mut found);
    found
}

/// [`selector_list_starts`] where the source is known to hold exactly one list.
fn selector_list_start(src: &str) -> u64 {
    match selector_list_starts(src).as_slice() {
        [one] => *one,
        other => panic!("expected exactly one SelectorList, found {}", other.len()),
    }
}

/// Every CSS comment in the wire, by value — the assertion the named-node collector cannot
/// make, since a comment is not a named node.
fn css_comment_values(src: &str) -> Vec<String> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let mut out = Vec::new();
    fn walk(node: &Value, out: &mut Vec<String>) {
        match node {
            Value::Object(fields) => {
                if fields.get("type").and_then(Value::as_str) == Some("CSSComment")
                    && let Some(v) = fields.get("value").and_then(Value::as_str)
                {
                    out.push(v.to_owned());
                }
                fields.values().for_each(|v| walk(v, out));
            }
            Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
            _ => {}
        }
    }
    walk(json.get("css").unwrap_or(&Value::Null), &mut out);
    out
}

/// A comment behind a boundary run still belongs to its juncture.
///
/// The run is stepped by `skip_boundary_whitespace`, which stops ON a comment exactly where
/// the plain whitespace skip does — so each loop's own comment arm keeps the disposition
/// (the stylesheet body registers one, a block pushes it as a child). Step the run *after*
/// that arm instead and the cursor lands on a comment the arm has already been passed: the
/// stylesheet body then REJECTS the document and a block's "skip unexpected token" tail eats
/// the comment silently. Both were live, in three of the four junctures below.
#[test]
fn a_comment_behind_a_boundary_run_still_reaches_its_juncture() {
    for (juncture, template) in COMMENT_AFTER_RUN_JUNCTURES {
        for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
            for run in [ch.to_owned(), format!("{ch} "), format!(" {ch}")] {
                let src = component(&template.replace("{T}", &run));
                assert_eq!(
                    css_comment_values(&src),
                    vec![" c ".to_owned()],
                    "{label} {juncture} (run {run:?}): the comment must survive the skip"
                );
            }
        }
    }
}

/// A boundary run is one run however its members are spelled, so a member followed by ASCII
/// whitespace must be skipped along with it.
///
/// `parseCss` reaches every juncture in the table through a single `allow_whitespace()`,
/// whose class holds both halves — so `<NBSP><SP>` is one skip there, and a reader that steps
/// only the non-ASCII half leaves an ASCII gap standing where a name is due. That is not a
/// cosmetic residue: at `[` and after a combinator the leftover gap is where a name was
/// expected, so tsv **rejected** input canonical accepts.
///
/// Both orders are asserted. Only member-then-ASCII was ever wrong — ASCII-then-member is
/// consumed by the ordinary whitespace skip that runs first — and pinning the two together is
/// what keeps the passing order a null control rather than an untested assumption.
#[test]
fn a_boundary_run_is_skipped_whole_whichever_order_its_halves_come_in() {
    for (juncture, template, (expected_type, expected_name)) in BOUNDARY_JUNCTURES {
        for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
            for run in [format!("{ch} "), format!(" {ch}"), format!("{ch}\t{ch}")] {
                let src = component(&template.replace("{T}", &run));
                let named = named_nodes(&src);
                assert!(
                    named
                        .iter()
                        .any(|(ty, name)| ty == expected_type && name == expected_name),
                    "{label} {juncture} (run {run:?}): expected a {expected_type} named \
                     {expected_name:?}, wire held {named:?}"
                );
            }
        }
    }
}

/// The same run, asked of a SPAN rather than a name: the selector list opens on the name.
///
/// The name-level assertion above cannot see this — a run the parser half-skips still yields
/// the right name, because the leftover ASCII gap is stepped by the next ordinary whitespace
/// skip. What it moves is the offset captured *before* that skip, which every enclosing node
/// inherits: the rule's `start`, its prelude's, and the complex selector's.
#[test]
fn a_half_skipped_run_would_move_the_selector_lists_own_start() {
    for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
        let src = component(&format!("<style>{ch} zz {{ color: red; }}</style>"));
        assert_eq!(
            selector_list_start(&src),
            utf16_offset_of(&src, "zz"),
            "{label} + SPACE at a selector-list start: the list must open on the name"
        );
    }
}

/// The printer keeps every member of the run, wherever in it they sit.
///
/// The parser skips them because `parseCss` skips them, but that is a statement about the
/// AST; dropping one from the OUTPUT is content loss the corpus safety check reads as
/// `content_lost`. prettier keeps the run verbatim from its first non-ASCII member on — the
/// ASCII head is indentation, which the printer regenerates — and tsv matches it there.
#[test]
fn the_printer_keeps_every_member_of_a_mixed_run() {
    for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
        // Not at offset 0 of the document, so a `<ZWNBSP>` here is an ordinary character
        // rather than a byte-order mark (which tsv strips by policy).
        // The last two are the two terms `is_boundary_whitespace` is a union of: a `<VT>` is
        // whitespace to the lexer but not to `char::is_ascii_whitespace`, and a `<NEL>` is
        // whitespace to the lexer but not to JS `\s`. Either term missing from the backward
        // scan stops it on that character and deletes the member behind it.
        for run in [
            format!("{ch} "),
            format!(" {ch}"),
            format!("{ch}\t{ch}"),
            format!("{ch}\u{b}"),
            format!("{ch}\u{85}"),
        ] {
            let src = component(&format!("<style>{run}zz {{ color: red; }}</style>"));
            let out = tsv_svelte::format_str(&src).expect("component should format");
            assert_eq!(
                out.matches(ch).count(),
                run.matches(ch).count(),
                "{label} (run {run:?}): every member must survive formatting — got {out:?}"
            );
            let again = tsv_svelte::format_str(&out).expect("output should re-format");
            assert_eq!(
                again, out,
                "{label} (run {run:?}): formatting must be a fixed point"
            );
        }
    }
}

/// ⚠️ **RATCHET.** An ASCII whitespace run *interior* to a preserved run keeps the author's
/// spelling; prettier respells it as a single space.
///
/// Nothing is lost either way — both are ASCII whitespace, and every non-ASCII member
/// survives — so this is a spelling difference, in a run no authored stylesheet contains.
/// It is the one place `preserved_boundary_ws` and prettier disagree, and it is pinned rather
/// than closed because collapsing it would give the printer two whitespace policies inside
/// one run, and prettier is not a coherent oracle in this corner anyway: at the `[` juncture
/// it DROPS the character outright (`[<NBSP>a]` → `[a]`) where tsv keeps it.
#[test]
fn an_interior_ascii_run_keeps_its_spelling() {
    let out = tsv_svelte::format_str(&component(
        "<style>\u{a0}\t\u{a0}zz { color: red; }</style>",
    ))
    .expect("component should format");
    assert!(
        out.contains("\u{a0}\t\u{a0}zz"),
        "the interior TAB is kept as written; prettier respells it as a space — got {out:?}"
    );
}
