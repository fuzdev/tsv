// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used, clippy::panic)]

//! Every `loc` in the Svelte wire is one of **two** line counts, and which one a node gets is
//! decided by the acorn parse it came from.
//!
//! Svelte's own positions (`locate-character`) open a line at `\n` and nothing else — the
//! spine, `name_loc`, the CSS `loc`, a `Program`'s own `loc`, and the identifiers
//! `read_identifier` builds. Everything acorn parses carries acorn's, which is the ECMAScript
//! class: `\n`, `\r`, `\r\n`, `<LS>`, `<PS>`. And acorn seeds that counter **once per parse**,
//! over whatever prefix Svelte prepared for it, so an island's `loc` is not simply "the
//! ECMAScript table's answer" either — see `tsv_ts::AcornSeed`.
//!
//! **Why a test rather than a fixture.** The `<LS>` / `<PS>` spellings are format-stable
//! inside a verbatim region and are pinned by
//! `tests/fixtures/svelte/syntax/whitespace/line_terminators_acorn_regions`, which is where
//! the region-by-region claims live. A raw `<CR>` cannot be a fixture *input* at all: every
//! parse-then-format entry point folds it to `<LF>` before it parses
//! (`tsv_lang::printing::normalize_carriage_returns`), so a document carrying one formats to
//! its `<LF>` twin and is not the fixed point F1 requires. So the lone-CR half of the class
//! lives here, expectations transcribed from the live modern Svelte parser
//! (`cargo run -p tsv_debug canonical_parse`) — the same arrangement, and the same weakness,
//! as `tests/comment_dedent_line_terminators.rs`.
//!
//! `\r\n` is the null control throughout, and it is not a formality: CRLF is a single
//! ECMAScript break holding a single LF, so it leaves the two classes agreeing at every
//! position. It is also what the whole mechanism is gated on — a source with no lone CR,
//! `<LS>` or `<PS>` never builds acorn's second line table (`LocationTracker::new_with_map`
//! reports that), so a CRLF document has to come out of the *unseeded* path unchanged.

use serde_json::Value;

/// Every `loc.start.line` in the wire, in emission order, paired with the path that reached
/// it — enough to say which region moved without pinning the whole document.
fn loc_lines(src: &str) -> Vec<(String, u64)> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the component");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let mut out = Vec::new();
    collect(&json, String::new(), &mut out);
    out
}

fn collect(node: &Value, path: String, out: &mut Vec<(String, u64)>) {
    match node {
        Value::Object(fields) => {
            if let Some(line) = fields
                .get("loc")
                .and_then(|loc| loc.get("start"))
                .and_then(|start| start.get("line"))
                .and_then(Value::as_u64)
            {
                let ty = fields.get("type").and_then(Value::as_str).unwrap_or("?");
                out.push((format!("{path}.{ty}"), line));
            }
            for (key, value) in fields {
                collect(value, format!("{path}.{key}"), out);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                collect(item, format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

/// The line of the one `loc` whose path ends in `suffix`.
fn line_of(src: &str, suffix: &str) -> u64 {
    let lines = loc_lines(src);
    let mut hits = lines.iter().filter(|(path, _)| path.ends_with(suffix));
    let (_, line) = hits
        .next()
        .unwrap_or_else(|| panic!("no `loc` at a path ending {suffix:?}; wire held {lines:?}"));
    assert!(
        hits.next().is_none(),
        "{suffix:?} should name one node; wire held {lines:?}"
    );
    *line
}

/// Every spelling of the line-terminator class, in the order each test lists them. `\r\n` is
/// in every list on purpose: it is the null control, and a shape that skipped it would assert
/// only that *something* moved, never that CRLF leaves the two classes agreeing.
const TERMINATORS: [&str; 4] = ["\n", "\r\n", "\r", "\u{2028}"];

/// Run one shape against each spelling of the class: `{T}` in `shape` is the terminator, and
/// each row pairs it with the line the oracle reports for the node at `suffix`.
///
/// The rows name their own terminator so each expectation reads beside the case it belongs to,
/// and the class check below is what keeps that from drifting into six different classes.
fn assert_each_terminator(shape: &str, suffix: &str, cases: [(&str, u64); 4]) {
    assert_eq!(
        cases.map(|(terminator, _)| terminator),
        TERMINATORS,
        "every shape must run the whole class, in one order"
    );
    for (terminator, expected) in cases {
        let src = shape.replace("{T}", terminator);
        assert_eq!(line_of(&src, suffix), expected, "terminator {terminator:?}");
    }
}

/// A `<script>` body is acorn's, and acorn lexes the terminator inside it — so every position
/// after one is a line further on than `locate-character` would put it.
#[test]
fn a_script_body_counts_the_ecmascript_class() {
    assert_each_terminator(
        "<script lang=\"ts\">\n\tconst a = `t1{T}`;\n\tconst b = 2;\n</script>\n",
        ".body[1].VariableDeclaration",
        [("\n", 4), ("\r\n", 4), ("\r", 4), ("\u{2028}", 4)],
    );
}

/// The same terminator ahead of the `<script>` does **not** move it: `read_script` hands acorn
/// the prefix blanked with `replace(/[^\n]/g, ' ')`, so only its LFs ever reached the parse.
///
/// This is the null control the naive fix fails — routing the island to a plain
/// ECMAScript-rule table counts a terminator acorn never saw.
#[test]
fn a_terminator_ahead_of_a_script_is_blanked_out_of_its_parse() {
    assert_each_terminator(
        "<p>text1{T}text2</p>\n<script lang=\"ts\">\n\tconst a = 1;\n</script>\n",
        ".body[0].VariableDeclaration",
        [("\n", 4), ("\r\n", 4), ("\r", 3), ("\u{2028}", 3)],
    );
}

/// A template expression is the opposite arm: `read_expression` hands acorn the **raw**
/// template, so a terminator anywhere ahead of the island's line counts.
#[test]
fn a_template_expression_counts_the_prefix_under_the_ecmascript_class() {
    assert_each_terminator(
        "<p>text1{T}text2</p>\n<p>{expr}</p>\n",
        ".expression.Identifier",
        [("\n", 3), ("\r\n", 3), ("\r", 3), ("\u{2028}", 3)],
    );
}

/// …but only ahead of its *line*. acorn seeds `lineStart` with `lastIndexOf("\n", startPos - 1)`
/// and then jumps straight to `startPos`, so a terminator between that LF and the tag is never
/// counted — it is neither in the prefix acorn measured nor in the region it lexed.
///
/// The second null control, and the one that says the seed is a per-parse fact rather than a
/// choice of table: here the ECMAScript class is the *wrong* answer even though the island is
/// acorn's.
#[test]
fn a_terminator_on_the_islands_own_line_is_skipped_by_the_seed() {
    assert_each_terminator(
        "<p>a</p>\n<p>text1{T}text2{expr}</p>\n",
        ".expression.Identifier",
        [("\n", 3), ("\r\n", 3), ("\r", 2), ("\u{2028}", 2)],
    );
}

/// A root `comments` entry carries the `loc` of whichever parse produced it, and this array is
/// emitted outside the tree walk — so the region has to be recoverable from the comment's own
/// position (`Root::acorn_regions`).
#[test]
fn a_script_comment_takes_its_scripts_seed() {
    assert_each_terminator(
        "<script lang=\"ts\">\n\tconst a = `t1{T}`;\n\t/* c1 */\n</script>\n",
        ".comments[0].Block",
        [("\n", 4), ("\r\n", 4), ("\r", 4), ("\u{2028}", 4)],
    );
}

/// An in-tag comment is Svelte's own reader, not acorn — it keeps `locate-character` (and is
/// the one comment shape whose `loc` carries a `character`), so no terminator class moves it
/// but `\n`.
#[test]
fn an_in_tag_comment_stays_on_locate_characters_lines() {
    assert_each_terminator(
        "<p>text1{T}text2</p>\n<div /* c1 */ data-attr=\"a\">text3</div>\n",
        ".comments[0].Block",
        [("\n", 3), ("\r\n", 3), ("\r", 2), ("\u{2028}", 2)],
    );
}
