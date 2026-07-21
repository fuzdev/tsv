// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! `:nth-*()` argument span end trimming in the CSS parser (Svelte-embedded).
//!
//! Svelte's `read_selector_list` (`.../read/style.js`) captures the selector
//! list's `end` **before** `allow_comment_or_whitespace`, so the `args`
//! SelectorList (and every node nested under it) ends at the last content token,
//! not at the closing `)`. tsv's `:nth-*()` path (`parse_nth_args`) previously set
//! the `Nth` node's span end to the `)` position, so any trailing whitespace
//! between the An+B value (or the `of S` selector) and `)` was wrongly absorbed
//! into `args.end` / `args.children[].end` / …`selectors[].end`.
//!
//! Not fixturable: prettier collapses `:nth-child(\n n \n)` → `:nth-child(n)`, so
//! `fixture_init` formats the trigger away and the collapsed form has no
//! divergence (the same reason the auto-close `end` offsets live in
//! `tests/svelte_autoclose.rs`). These root tests pin the trimmed offsets offline,
//! transcribed from the live modern Svelte parser (`tsv_debug canonical_parse`).

use serde_json::Value;

/// Parse `src`, convert to the wire AST, and return the first
/// `PseudoClassSelector`'s `args` object.
fn nth_args(src: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_svelte::Interner::new();
    let ast = tsv_svelte::parse(src, &arena, &mut interner).expect("parser should accept the CSS");
    let json = tsv_svelte::convert_ast_json(&ast, src, &interner);
    find_pseudo_args(&json).expect("a PseudoClassSelector with args")
}

fn find_pseudo_args(node: &Value) -> Option<Value> {
    match node {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("PseudoClassSelector")
                && let Some(args) = map.get("args")
            {
                return Some(args.clone());
            }
            map.values().find_map(find_pseudo_args)
        }
        Value::Array(items) => items.iter().find_map(find_pseudo_args),
        _ => None,
    }
}

fn end_of(node: &Value) -> i64 {
    node["end"].as_i64().unwrap_or(-1)
}

/// Assert that the `args` SelectorList and every node on the single-selector chain
/// under it (ComplexSelector → RelativeSelector) share `expected_end` — the trimmed
/// content end the oracle produces, not the `)` position.
fn assert_chain_end(args: &Value, expected_start: i64, expected_end: i64) {
    assert_eq!(args["type"], "SelectorList");
    assert_eq!(args["start"].as_i64(), Some(expected_start), "args.start");
    assert_eq!(end_of(args), expected_end, "args.end");
    let complex = &args["children"][0];
    assert_eq!(complex["type"], "ComplexSelector");
    assert_eq!(end_of(complex), expected_end, "args.children[].end");
    let relative = &complex["children"][0];
    assert_eq!(relative["type"], "RelativeSelector");
    assert_eq!(
        end_of(relative),
        expected_end,
        "args.children[].children[].end"
    );
}

/// Multiline `:nth-child(\n n \n)` — the shape prettier can't preserve. The `n`
/// token ends at byte 22; `)` is at 23. args must end at 22.
#[test]
fn multiline_nth_child_trims_trailing_whitespace() {
    let args = nth_args("<style>a:nth-child(\n\tn\n) {color: red}</style>");
    assert_chain_end(&args, 21, 22);
    // the innermost `Nth` selector is trimmed too
    let nth = &args["children"][0]["children"][0]["selectors"][0];
    assert_eq!(nth["type"], "Nth");
    assert_eq!(nth["value"], "n");
    assert_eq!(end_of(nth), 22, "Nth.end");
}

/// Single-line `:nth-child(2n + 1  )` with trailing spaces. `2n + 1` ends at byte
/// 25; `)` is at 27. args must end at 25.
#[test]
fn nth_child_trailing_spaces_trims() {
    let args = nth_args("<style>b:nth-child(2n + 1  ) {color: red}</style>");
    assert_chain_end(&args, 19, 25);
    let nth = &args["children"][0]["children"][0]["selectors"][0];
    assert_eq!(nth["value"], "2n + 1");
    assert_eq!(end_of(nth), 25, "Nth.end");
}

/// `:nth-child(2n of .d  )` — the `of S` selector list. `.d` ends at byte 27; `)`
/// is at 29. args must end at 27. (The internal `of`-nesting shape is a separate,
/// pre-existing divergence; here we only pin the trimmed chain end.)
#[test]
fn nth_child_of_selector_trailing_spaces_trims() {
    let args = nth_args("<style>c:nth-child(2n of .d  ) {color: red}</style>");
    assert_chain_end(&args, 19, 27);
}
