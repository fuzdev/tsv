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
//! **The juncture inventory.** `parseCss` has exactly two skips —
//! `parser.allow_whitespace()` and the `allow_comment_or_whitespace` loop around it — and the
//! tables below are one entry per position either of them occupies: a selector-list start,
//! after a `,`, after a `;`, each child of a block, the compound break after every simple
//! selector, a combinator's two gaps, a pseudo-argument list's start and its `)`, and the
//! attribute selector's matcher→value, value→flags and flags→`]` gaps. Two positions are
//! deliberately absent, because `read_identifier` reaches them first and the code point is
//! CONTENT there: a name glued to its `.`/`#`/`:`/`|`/`@` sigil, and a run glued to the end of
//! a name (`[a<NBSP>]`, `.a<NBSP>b` — one name on both sides). The second of those flips once
//! ASCII whitespace has closed the name, which is its own test.
//!
//! ⚠️ **`allow_comment_or_whitespace` is a LOOP**, so a comment does not end the juncture —
//! whitespace, comment, whitespace, … is one gap. tsv splits the comment's *disposition* out
//! to each site (register it, push it as a block child, leave it for the caller), so every
//! such site owes the boundary skip on BOTH sides of the comment it consumes. A run behind a
//! comment was a parse error at four selector junctures and silently DROPPED a selector at
//! two more.
//!
//! **A skip and a preservation are one change.** Every juncture the parser steps a run at is
//! a juncture the printer must put it back at, and the printer regenerates most of these gaps
//! from parts rather than copying source — so a new skip with no matching
//! `preserved_boundary_ws` / `boundary_ws_in_gap` turns a graceful over-rejection into
//! content loss, which is the worse trade. Two shapes of claim partition every gap: the
//! backward scan takes the run CONTIGUOUS with the anchor (and needs a FLOOR wherever the
//! previous node's own span can end inside the run, or it emits a character the preceding
//! NAME already carried), and a forward sweep takes what a comment or a symbol strands
//! earlier in the gap (`Printer::boundary_ws_in_gap_before_anchor`, bounded at the
//! contiguous run's start so the two never claim one character twice). Three standing
//! corollaries, each of which has been a live bug: a gap gate keyed on COMMENTS blinds the
//! preservation that shares it (`::part()`'s `join(" ")`); a printer scan that locates a part
//! by stepping trivia owes the boundary class too, or it reports the RUN as the part's start
//! (the attribute rebuild, which mislocated an offset into the middle of a character and
//! panicked); and a run emitted flush against a NAME glues into it, so every claim whose left
//! neighbour can be a name puts an ASCII space ahead of it — which is **five** claims, not
//! one (`[a<HERE>]`, a list's `,`, an explicit combinator's symbol, a pseudo-argument list's
//! `)`, and the commented attribute rebuild's interior gaps), all answering
//! `Printer::name_run_separator` / `name_run_separator_after`. Pinned by
//! `a_preserved_run_never_moves_a_names_boundary`, whose templates are one per EMITTER.
//!
//! A fourth corollary runs the other way, and `a_rebuilt_head_and_its_claim_never_both_keep_the_run`
//! is its gate: a head the printer REBUILDS from trimmed text must not keep the run the claim
//! beside it restores. `str::trim` is Unicode `White_Space`, which matches this class on every
//! member **but `<ZWNBSP>`** — so `color<ZWNBSP>:` and `@container name <ZWNBSP>(…)` held the
//! character in both places and doubled it every pass, while any of the other four members
//! looked clean. Iterate the class; never probe one member of it.
//!
//! **Residue, deliberately not closed here.** Two things still disagree with `parseCss`
//! about a boundary run, neither of them a skip juncture — each is a *reader* whose own
//! whitespace class is narrower than the one `parseCss` uses in the same place, so closing
//! them changes what a token spans rather than where a skip goes:
//!
//! - a **trailing** run inside an attribute selector's value or case flag. `parseCss` ends
//!   both tokens at the run (`read_attribute_value`'s `REGEX_CLOSING_BRACKET` is JS `\s`, and
//!   it trims; `REGEX_ATTRIBUTE_FLAGS` reads letters only), where tsv's lexer reads one
//!   identifier whose tail the head-anchored skip cannot reach — so the run rides into the
//!   `value` (`[a=b<NBSP>]`), swallows the flag behind it (`[a=b<NBSP>i]`), or stands where
//!   the `]` is due and the document is rejected (`[a=b i<NBSP>]`).
//!
//! The **An+B scanner** used to be the second: ASCII where `REGEX_NTH_OF` is a JS regex, so
//! `:nth-child(2n<NBSP>)` and its tail gaps were rejected and `(even<NBSP>)` demoted to a type
//! selector. It is CLOSED — one scanner serves two grammars behind a `spec` flag, and each now
//! has its own class (Svelte's JS `\s`, and the parser's boundary class for the `:nth-*()`
//! term, whose terminator is one of these junctures) — and
//! `an_an_plus_b_juncture_steps_the_boundary_class` asks the whole class of it.
//!
//! A third — a `<ZWNBSP>` leading or trailing a declaration VALUE — is CLOSED, and its
//! assertion lives in `a_declarations_boundary_trims_are_the_js_class`: the wire's own trims
//! are JS `\s` now (`ast/convert/mod.rs`'s `trim_wire*`, mirroring `read_value`'s
//! `value.trim()`), where they used to be `str::trim` — which kept a `<ZWNBSP>` `read_value`
//! drops and deleted a `<NEL>` it keeps. ⚠️ That is the WIRE's trim; the printer's
//! property→colon trim is a second reader of the same seam and took the same correction from
//! the other side (`trim_property_part`) — see the fourth corollary above, which is what
//! happens when only one of them moves.
//!
//! The `<NEL>` (U+0085) gap is tracked separately below, with the raw-scan family it belongs
//! to rather than with this class.
//!
//! **The printer's own residue is a different, shorter list**, ratcheted by
//! `the_printers_remaining_drops_are_a_tracked_gap`: the stylesheet's own trailing whitespace
//! (the outermost gap has no following construct at all, and a Svelte `<style>` host trims
//! the island's tail before writing it), and a run inside a **comment-bearing**
//! property→colon gap, where the property name is reconstructed from its parts and the gap's
//! whitespace normalizes with it. Everywhere else — every selector juncture, every rebuilt
//! block-child head (a declaration's property, an at-rule's `@`, a comment's `/*`), a block's
//! tail before its `}`, and every gap of a `@supports` / `@container` condition prelude — the
//! character comes back.
//!
//! One **over-rejection** is ratcheted with them: `a { color <NBSP>: red }` and
//! `a { color /* c */<NBSP>: red }`, where the declaration-vs-rule byte scan
//! (`peek_significant_kind_bytes`) stops on the run, declines, and the token lookahead behind
//! it reads the run as the identifier that should have been a `:`. `parseCss` reads the
//! property raw and trims it, so both are declarations there.
//!
//! ⚠️ Two members of the class are LINE TERMINATORS to the shared line table (`<LS>`, `<PS>`),
//! so preserving one beside a regenerated newline made the next pass read a blank line where
//! the author wrote none. `Printer::has_blank_line_between` confirms the table's positive
//! answer against a class that stops at `<LF>` / `<CR>`; pinned by
//! `a_preserved_line_terminator_fabricates_no_blank_line`. That exclusion is a **deliberate
//! divergence** — prettier's `isNextLineEmpty` counts `<LS>` / `<PS>` — and it is the ONLY
//! way the confirm departs from prettier: everything else about it is that function
//! transcribed, `<FF>` included, which `the_blank_line_rule_is_prettiers_walk_not_a_terminator_count`
//! pins. ⚠️ Do not "fix" the confirm against css-syntax-3 §3.3, whose `<FF>`-is-a-newline
//! rule reads like the obvious authority and is the wrong oracle for a cosmetic blank line.

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
const BOUNDARY_JUNCTURES: [(&str, &str, (&str, &str)); 19] = [
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
    // `read_selector` runs `allow_comment_or_whitespace` after EVERY simple selector, so a
    // run ends the compound and reads as a descendant combinator. It only ever reaches this
    // position after a simple selector that is not an identifier: an identifier's own
    // trailing run is swallowed by `read_identifier` on both sides (`.a<NBSP>b` is one
    // name), which is why every spelling below ends in `*`, `&`, `)` or `]`.
    (
        "the compound break after a `*`",
        "<style>*{T}b { color: red; }</style>",
        ("TypeSelector", "b"),
    ),
    (
        "the compound break after a `&`",
        "<style>a { &{T}b { color: red; } }</style>",
        ("TypeSelector", "b"),
    ),
    (
        "the compound break after a pseudo-class's `)`",
        "<style>:is(a){T}b { color: red; }</style>",
        ("TypeSelector", "b"),
    ),
    (
        "the compound break after an attribute selector's `]`",
        "<style>[a]{T}b { color: red; }</style>",
        ("TypeSelector", "b"),
    ),
    // A pseudo-class argument list is `read_selector_list` again, so it brings every juncture
    // that reader has: its own start, and the gap before its `)`.
    (
        "a pseudo-class argument list's start",
        "<style>:is({T}b) { color: red; }</style>",
        ("TypeSelector", "b"),
    ),
    (
        "a forgiving argument list's start",
        "<style>:where({T}b) { color: red; }</style>",
        ("TypeSelector", "b"),
    ),
    (
        "a relative argument list's start",
        "<style>a:has({T}b) { color: red; }</style>",
        ("TypeSelector", "b"),
    ),
    (
        "a `::part()` argument list's start",
        "<style>::part({T}b) { color: red; }</style>",
        ("TypeSelector", "b"),
    ),
    // The attribute selector's own `allow_whitespace()` junctures. The name→matcher gap is
    // absent on purpose: `read_identifier` reaches a run GLUED to the name first, so
    // `[a<NBSP>]` is the name `a<NBSP>` on both sides — the boundary reading is reachable
    // there only once ASCII whitespace has closed the name, which
    // `a_boundary_run_is_skipped_whole_whichever_order_its_halves_come_in` covers.
    (
        "an attribute selector's matcher→value gap",
        "<style>[a={T}b] { color: red; }</style>",
        ("AttributeSelector", "a"),
    ),
    (
        "an attribute selector's value→`]` gap",
        "<style>[a='b'{T}] { color: red; }</style>",
        ("AttributeSelector", "a"),
    ),
    (
        "an attribute selector's value→flags gap",
        "<style>[a='b'{T}i] { color: red; }</style>",
        ("AttributeSelector", "a"),
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
const COMMENT_AFTER_RUN_JUNCTURES: [(&str, &str); 10] = [
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
    // …and the mirror spelling, a run BEHIND the comment. `allow_comment_or_whitespace` is a
    // LOOP — whitespace, comment, whitespace, … — so both sides of a comment are the same
    // juncture; a reader that steps the run only ahead of the comment leaves it standing
    // where the `,`, the `{` or the `)` is due. Every one of these was a parse error, and
    // the two pseudo-argument spellings silently DROPPED a selector.
    (
        "behind a comment before a comma",
        "<style>a /* c */{T}, b { color: red; }</style>",
    ),
    (
        "behind a comment before a rule's brace",
        "<style>a /* c */{T}{ color: red; }</style>",
    ),
    (
        "behind a comment glued to a compound",
        "<style>.a/* c */{T}{ color: red; }</style>",
    ),
    (
        "behind a comment before a pseudo-argument's `)`",
        "<style>:is(b /* c */{T}) { color: red; }</style>",
    ),
    (
        "behind a comment before a pseudo-argument's comma",
        "<style>:is(b /* c */{T}, c) { color: red; }</style>",
    ),
    (
        "behind a comment before an at-rule block's brace",
        "<style>@media screen { a /* c */{T}{ color: red } }</style>",
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

/// The junctures where `read_identifier` and `allow_whitespace()` each win for a different
/// SPELLING of the same run — so neither table above can hold them.
///
/// Both readers sit at the same position; which one reaches the run decides. Glued to the
/// name (`[a<NBSP>]`, `::part(a<NBSP>b)`) the identifier reader gets there first and the code
/// point is content — on both sides, so the two agree and nothing is skipped. Put an ASCII
/// space in front of it (`[a <NBSP>]`) and the name has already ended: the run is now a
/// separate token, `parseCss` steps it with the rest of its `allow_whitespace()`, and a
/// reader that steps only the `<whitespace-token>` leaves it standing where the `]` or the
/// next name is due. That was a parse error on input canonical accepts.
#[test]
fn a_separated_run_is_a_boundary_where_a_glued_one_is_content() {
    for (juncture, template) in [
        (
            "an attribute selector's name",
            "<style>[a{RUN}] { color: red; }</style>",
        ),
        (
            "a `::part()` name",
            "<style>::part(a{RUN}b) { color: red; }</style>",
        ),
    ] {
        for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
            // Glued: the run is part of the name, on both sides.
            let glued = component(&template.replace("{RUN}", ch));
            let named = named_nodes(&glued);
            assert!(
                named.iter().any(|(_, name)| name.contains(ch)),
                "{label} {juncture}: a GLUED run is identifier content, wire held {named:?}"
            );
            // Separated by ASCII whitespace: the run is a boundary, and the document parses.
            for run in [format!(" {ch}"), format!(" {ch} "), format!("\t{ch}")] {
                let src = component(&template.replace("{RUN}", &run));
                assert!(
                    parses(&src),
                    "{label} {juncture} (run {run:?}): a run ASCII whitespace has separated \
                     from the name is a boundary — canonical accepts this"
                );
            }
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
        .chain([
            (
                "inside an identifier",
                "<style>.a{T}b { color: red; }</style>",
            ),
            // The An+B junctures, one per grammar: the `:nth-*()` scanner steps the parser's
            // boundary class (`<NEL>` in it, deliberately — the same over-acceptance as
            // every juncture above), and inside `:is()` the lexer's own whitespace read
            // carries the character past the term.
            (
                "an An+B term's terminator",
                "<style>:nth-child(2n{T}) { color: red; }</style>",
            ),
            (
                "a Svelte-grammar An+B term's terminator",
                "<style>:is(2n{T}) { color: red; }</style>",
            ),
        ])
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

/// A comment-then-run juncture must not lose a SELECTOR either.
///
/// The sibling of `a_comment_behind_a_boundary_run_still_reaches_its_juncture`, asserting the
/// other thing that went missing: with the run standing where the `,` was due, the forgiving
/// list decided it had ended and every selector past that point was DROPPED — silently, with
/// the document still parsing. A comment count cannot see that, and a name count is what can.
#[test]
fn a_comment_behind_a_boundary_run_loses_no_selector() {
    for (juncture, template, expected) in [
        (
            "a forgiving argument list",
            "<style>:is(b /* c */{T}, c, d) { color: red; }</style>",
            ["b", "c", "d"],
        ),
        (
            "a complex selector list",
            "<style>a /* c */{T}, b, d { color: red; }</style>",
            ["a", "b", "d"],
        ),
    ] {
        for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
            for run in [ch.to_owned(), format!("{ch} "), format!(" {ch}")] {
                let src = component(&template.replace("{T}", &run));
                let named = named_nodes(&src);
                for want in expected {
                    assert!(
                        named
                            .iter()
                            .any(|(ty, name)| ty == "TypeSelector" && name == want),
                        "{label} {juncture} (run {run:?}): selector {want:?} was dropped — \
                         wire held {named:?}"
                    );
                }
            }
        }
    }
}

/// The two span-level junctures a name-level assertion cannot see, both closed.
///
/// A pseudo-class argument list's own `start` is `read_selector_list`'s, taken AFTER its
/// `allow_comment_or_whitespace` — so the list opens on the name, not on the run. And a
/// descendant combinator's `end` is where `read_combinator`'s `allow_whitespace()` left off,
/// which is the same place: the whole run, not its ASCII half. Both were off by the run's
/// length, and both are invisible to every other assertion in this file.
#[test]
fn a_run_moves_neither_an_argument_lists_start_nor_a_combinators_end() {
    for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
        for template in [
            "<style>:is({T}b) { color: red; }</style>",
            "<style>:where({T}b) { color: red; }</style>",
            "<style>:not({T}b) { color: red; }</style>",
            "<style>:global({T}b) { color: red; }</style>",
            "<style>::slotted({T}b) { color: red; }</style>",
            "<style>:nth-child({T}2n) { color: red; }</style>",
            "<style>a:has({T}b) { color: red; }</style>",
        ] {
            let src = component(&template.replace("{T}", ch));
            let starts = pseudo_args_starts(&src);
            let want = utf16_offset_of(&src, ch) + ch.encode_utf16().count() as u64;
            assert_eq!(
                starts,
                vec![want],
                "{label} {template}: the argument list opens past the run"
            );
        }

        // `a <NBSP>b`: the combinator ends where the name begins, not where the run does.
        let src = component(&format!("<style>a {ch}b {{ color: red; }}</style>"));
        assert_eq!(
            combinator_ends(&src),
            vec![utf16_offset_of(&src, "b")],
            "{label}: the descendant combinator covers the whole run"
        );
    }
}

/// Every `start` of a pseudo-class/element argument list in the wire, in emission order.
fn pseudo_args_starts(src: &str) -> Vec<u64> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let mut out = Vec::new();
    fn walk(node: &Value, out: &mut Vec<u64>) {
        match node {
            Value::Object(fields) => {
                if let Some(args) = fields.get("args").and_then(Value::as_object) {
                    out.extend(args.get("start").and_then(Value::as_u64));
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

/// Every `Combinator`'s `end` in the wire, in emission order.
fn combinator_ends(src: &str) -> Vec<u64> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let mut out = Vec::new();
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
    walk(json.get("css").unwrap_or(&Value::Null), &mut out);
    out
}

/// A preserved `<LS>` / `<PS>` must not FABRICATE a blank line.
///
/// The two are line terminators to the shared (ECMAScript) line table and *not* to CSS
/// (css-syntax-3 §3.3), and the printer preserves them into its output while regenerating the
/// line break beside them — so `a {…}<LS>b {…}`, one line break by either reading, comes back
/// as `}\n<LS>b`, which the wider class counts as two. Read that way the next pass inserts a
/// blank line, and the document formats two ways.
///
/// These are the only members of the class that can do it, and no fixture can carry one (a fixture
/// input is prettier-formatted, and prettier drops these characters), so the assertion lives
/// here. `Printer::has_blank_line_between` confirms the table's positive answer against a
/// terminator class that excludes `<LS>` / `<PS>`, which is what makes preservation and
/// blank-line preservation compatible — and is a **deliberate divergence**, since prettier
/// does count them (`:is` `a {}<LS><LS>b {}` blanks there and not here). Cataloged in
/// [conformance_prettier_css.md](../docs/conformance_prettier_css.md); its sibling
/// `the_blank_line_rule_is_prettiers_walk_not_a_terminator_count` pins the part tsv does
/// follow.
#[test]
fn a_preserved_line_terminator_fabricates_no_blank_line() {
    for (label, ch) in [("LS", "\u{2028}"), ("PS", "\u{2029}")] {
        for template in [
            "<style>a { color: red; }{T}b { color: red; }</style>",
            "<style>a { color: red;{T}top: 0; }</style>",
            "<style>a { color: red; }{T}@media screen { b { color: red } }</style>",
            "<style>a { color: red; }{T}/* c */</style>",
        ] {
            let src = component(&template.replace("{T}", ch));
            let out = tsv_svelte::format_str(&src).expect("component should format");
            assert_eq!(
                out.matches(ch).count(),
                1,
                "{label} {template}: the terminator survives exactly once — got {out:?}"
            );
            let again = tsv_svelte::format_str(&out).expect("output should re-format");
            assert_eq!(
                again, out,
                "{label} {template}: a preserved terminator must not grow a blank line"
            );
        }
    }
}

/// The blank-line rule is prettier's WALK, not a count of terminators.
///
/// `isNextLineEmpty` (prettier `src/utilities/is-next-line-empty.js`) skips `,; \t`, takes
/// **one** terminator, skips `" \t"`, and requires a second — so a character in neither set
/// ENDS the search instead of being stepped over. `<FF>` is such a character: it is not a
/// terminator to prettier and not in the `" \t"` between them, so `}\n<FF>\n{` is one line
/// there. css-syntax-3 §3.3 says the opposite (input preprocessing folds `<FF>` to `<LF>`),
/// and reading the spec into a **cosmetic** decision the formatter oracle owns was tsv's whole
/// disagreement with prettier over ASCII gaps — these three shapes, which a count cannot tell
/// apart from `}\n\n{`.
///
/// ⚠️ Not a fixture, for the usual reason: prettier normalizes the gap away, so none of these
/// is a formatting fixed point AS AUTHORED. The `<FF>` spelling additionally cannot survive a
/// fixture file at all — `fixture_init` formats through prettier, which freezes a raw `<FF>`
/// into its output in some positions and drops it in others.
#[test]
fn the_blank_line_rule_is_prettiers_walk_not_a_terminator_count() {
    // A `<FF>` between (or before) the two newlines ends prettier's walk — one line, not two.
    for gap in [
        "\u{c}",
        "\u{c}\u{c}",
        "\n\u{c}\n",
        "\u{c}\n\n",
        "\n\u{c}\u{c}\n",
        "\n\u{c}",
    ] {
        let src = component(&format!(
            "<style>a {{ color: red; }}{gap}b {{ color: red; }}</style>"
        ));
        let out = tsv_svelte::format_str(&src).expect("component should format");
        assert!(
            !out.contains("}\n\n\tb"),
            "gap {gap:?}: a <FF> ends prettier's walk, so this is one line — got {out:?}"
        );
    }
    // …and the spellings it does not end still blank, so the rule is not "any <FF> anywhere".
    for gap in ["\n\n", "\n \n", "\n\t\n", "\n \t \n", "\n\n\n"] {
        let src = component(&format!(
            "<style>a {{ color: red; }}{gap}b {{ color: red; }}</style>"
        ));
        let out = tsv_svelte::format_str(&src).expect("component should format");
        assert!(
            out.contains("}\n\n\tb"),
            "gap {gap:?}: only `\" \\t\"` may sit between the two terminators, and this gap \
             holds nothing else — got {out:?}"
        );
    }
}

/// ⚠️ **RATCHET for the printer's residue.** Two positions still DROP a boundary run the
/// parser skipped, and one shape is still over-REJECTED. They are enumerated in this module's
/// doc; asserted here so "documented but unchecked" cannot quietly become "documented and
/// wrong".
///
/// Neither drop is a selector juncture, and neither has a following construct in the same
/// node to hang the run against. The over-rejection is the declaration-vs-rule byte scan's
/// own whitespace class (`peek_significant_kind_bytes`), ASCII where `parseCss` reads the
/// property raw and `.trim()`s it — closing it means widening that scan and the token
/// lookahead behind it together, which is a change to what a token spans rather than to where
/// a claim goes.
#[test]
fn the_printers_remaining_drops_are_a_tracked_gap() {
    // ── D1: a run inside a COMMENT-BEARING property→colon gap ────────────────────────
    // The property name is reconstructed there (`extract_property_name` re-joins the name
    // and its comments with single spaces), so the gap's whitespace normalizes and a
    // boundary member goes with it. The comment-free spelling is claimed and kept.
    for (case, style, kept) in [
        (
            "a comment-bearing property gap",
            "<style>a { color\u{a0} /* c */ : red; }</style>",
            false,
        ),
        (
            "the comment-free spelling of the same gap",
            "<style>a { color\u{a0}: red; }</style>",
            true,
        ),
    ] {
        let src = component(style);
        let out = tsv_svelte::format_str(&src).expect("component should format");
        assert_eq!(
            out.contains('\u{a0}'),
            kept,
            "{case}: expected kept={kept} — if that changed, re-pin this ratchet (got {out:?})"
        );
        let again = tsv_svelte::format_str(&out).expect("output should re-format");
        assert_eq!(again, out, "{case}: the answer is at least a fixed point");
    }

    // ── D2: the stylesheet's own trailing whitespace ──────────────────────────────────
    // The outermost gap has no following construct at all, and under a Svelte `<style>` the
    // host trims the island's tail before writing it (`formatted_css.trim_end()`, whose
    // Unicode class takes these members) — so a claim here would be undone one layer up.
    let src = component("<style>a { color: red; }\u{a0}</style>");
    let out = tsv_svelte::format_str(&src).expect("component should format");
    assert!(
        !out.contains('\u{a0}'),
        "the stylesheet's trailing run is still dropped — if that changed, re-pin this \
         ratchet (got {out:?})"
    );

    // ── R4: the declaration-vs-rule lookahead's ASCII class ──────────────────────────
    // `parseCss` reads the property raw to the `:` and trims it, so all three parse there
    // with the property `color`; tsv's byte scan stops on the run, declines, and the token
    // lookahead behind it reads the run as an identifier — no `:` follows the name, so the
    // child is parsed as a nested rule and the document is REJECTED.
    for (case, style) in [
        (
            "a run separated from the property",
            "<style>a { color \u{a0}: red; }</style>",
        ),
        (
            "a run behind a property-gap comment",
            "<style>a { color /* c */\u{a0}: red; }</style>",
        ),
    ] {
        assert!(
            !parses(&component(style)),
            "{case}: tsv over-REJECTS this and canonical accepts it — if that changed, \
             re-pin this ratchet"
        );
    }
}

/// ⚠️ **RATCHET for the residue.** One family still disagrees with `parseCss` about a
/// boundary run. It is enumerated in this module's doc; asserted here so "documented but
/// unchecked" cannot quietly become "documented and wrong" — a prose-only gap is exactly the
/// kind that outlives the code it describes.
///
/// It is not a whitespace-SKIP juncture, which is why the sweep above left it:
/// `skip_boundary_whitespace` steps a run off the HEAD of the current token, and this is a
/// reader whose own whitespace class is narrower than the one `parseCss` uses in the same
/// place. Closing it means changing what a token spans, not where a skip goes. (The An+B
/// scanner was the second such reader, and was closed exactly that way — see
/// `an_an_plus_b_juncture_steps_the_boundary_class`.)
#[test]
fn the_remaining_junctures_are_a_tracked_gap() {
    // ── R1: a TRAILING run inside an attribute value or case flag ────────────────────
    // `read_attribute_value` stops on JS `\s` (`REGEX_CLOSING_BRACKET`) and trims what is
    // left, and `REGEX_ATTRIBUTE_FLAGS` reads letters only — so canonical ends both tokens
    // at the run. tsv reads one identifier token, whose tail the head-anchored boundary skip
    // cannot reach, so the run rides INTO the value (and swallows the flag behind it), or
    // stands where the `]` is due and the document is rejected outright.
    for (case, style, value) in [
        (
            "a bare value's trailing run",
            "<style>[a=b\u{a0}] { color: red; }</style>",
            "b\u{a0}",
        ),
        (
            "a bare value's trailing run, swallowing the flag",
            "<style>[a=b\u{a0}i] { color: red; }</style>",
            "b\u{a0}i",
        ),
    ] {
        let named = named_nodes(&component(style));
        assert!(
            named.iter().any(|(ty, _)| ty == "AttributeSelector"),
            "{case}: expected an AttributeSelector, wire held {named:?}"
        );
        assert_eq!(
            attribute_values(&component(style)),
            vec![value.to_owned()],
            "{case}: canonical stops the value at the run; tsv keeps it — if that changed, \
             re-pin this ratchet"
        );
    }
    for (case, style) in [
        (
            "a bare value's flag, trailing run",
            "<style>[a=b i\u{a0}] { color: red; }</style>",
        ),
        (
            "a quoted value's flag, trailing run",
            "<style>[a='b' i\u{a0}] { color: red; }</style>",
        ),
    ] {
        assert!(
            !parses(&component(style)),
            "{case}: tsv over-REJECTS this and canonical accepts it — if that changed, \
             re-pin this ratchet"
        );
    }
}

/// The An+B junctures step the same class every other juncture does — asked of the whole
/// class, because two grammars sit behind one scanner and parted on it: Svelte's
/// `REGEX_NTH_OF` (JS `\s`, the `:is()` reading) and the css-syntax-3 `<an+b>` microsyntax
/// (`<whitespace-token>`, ASCII — the `:nth-*()` reading), which tsv reads with the parser's
/// own boundary class, so a `:nth-*()` term meets its `)` or its `of` across the same skip
/// every other juncture has. Formerly R2 of the ratchet above: the ASCII scanner demoted the
/// argument to the selector-list path, where the lexer's `1<NBSP>` dimension was rejected — a
/// tsv over-rejection of input both oracles accept — and demoted `even<NBSP>` to a type
/// selector.
///
/// Wire AND format, since the two halves are one change (a skip added to the parser owes
/// the printer a claim): the `Nth.value` is what canonical reads, the character survives the
/// format, and the output is its own fixed point. The `:nth-*()` `of` cases carry tsv's
/// nested shape (`nth_child_of`), so the value alone is asserted there; the `:is()` fold
/// keeps the run inside the value, as Svelte's regex does.
#[test]
fn an_an_plus_b_juncture_steps_the_boundary_class() {
    for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
        for (case, template, value) in [
            // The `:nth-*()` term: its terminator, its tail's two gaps, both sides of `of`,
            // and the `)` gap after `S`.
            (
                "before the terminator",
                "<style>:nth-child(2n{T}) { color: red; }</style>",
                "2n",
            ),
            (
                "a keyword before the terminator",
                "<style>:nth-child(even{T}) { color: red; }</style>",
                "even",
            ),
            (
                "before the `+` of a tail",
                "<style>:nth-child(2n{T}+ 1) { color: red; }</style>",
                "2n{T}+ 1",
            ),
            (
                "after the `+` of a tail",
                "<style>:nth-child(2n +{T}1) { color: red; }</style>",
                "2n +{T}1",
            ),
            (
                "before `of`",
                "<style>:nth-child(2n{T}of .b) { color: red; }</style>",
                "2n",
            ),
            (
                "after `of`",
                "<style>:nth-child(2n of{T}.b) { color: red; }</style>",
                "2n",
            ),
            (
                "after `S`",
                "<style>:nth-child(2n of .b {T}) { color: red; }</style>",
                "2n",
            ),
            // Svelte's own grammar, inside `:is()`.
            (
                "`:is()` before the terminator",
                "<style>:is(2n{T}) { color: red; }</style>",
                "2n",
            ),
            (
                "`:is()` before a `,`",
                "<style>:is(2n{T}, .b) { color: red; }</style>",
                "2n",
            ),
            (
                "`:is()` around the operator",
                "<style>:is(2n{T}+{T}1) { color: red; }</style>",
                "2n{T}+{T}1",
            ),
            (
                "`:is()` after `of`",
                "<style>:is(2n of{T}.b) { color: red; }</style>",
                "2n of{T}",
            ),
        ] {
            let src = component(&template.replace("{T}", ch));
            assert_eq!(
                wire_field_values(&src, "Nth", "value"),
                vec![value.replace("{T}", ch)],
                "{label} {case}: the run is the juncture's, not the term's"
            );
            let out = tsv_svelte::format_str(&src).expect("component should format");
            assert!(
                out.contains(ch),
                "{label} {case}: the character must survive the format; got {out:?}"
            );
            assert_eq!(
                tsv_svelte::format_str(&out).expect("output should format"),
                out,
                "{label} {case}: the output must be its own fixed point"
            );
        }
    }
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
    // The shared wire walk, not a private one: the residue ratchet above reads a
    // `Declaration`'s `value` through `declaration_values` too, and two spellings of "read
    // this field off the wire" is how the two come to disagree about which subtree they walk.
    let value_of = |style: &str| declaration_values(&component(style));

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

/// Every `AttributeSelector`'s `value` in the wire, in emission order.
fn attribute_values(src: &str) -> Vec<String> {
    wire_field_values(src, "AttributeSelector", "value")
}

/// Every `Declaration`'s `value` in the wire, in emission order.
fn declaration_values(src: &str) -> Vec<String> {
    wire_field_values(src, "Declaration", "value")
}

/// Every string `field` of every `ty`-typed node in the CSS wire, in emission order.
///
/// The `named_nodes` collector reports a node by its NAME; these two residues are about a
/// node's `value`, which is a different key on the same walk.
fn wire_field_values(src: &str, ty: &str, field: &str) -> Vec<String> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let mut out = Vec::new();
    fn walk(node: &Value, ty: &str, field: &str, out: &mut Vec<String>) {
        match node {
            Value::Object(fields) => {
                if fields.get("type").and_then(Value::as_str) == Some(ty)
                    && let Some(v) = fields.get(field).and_then(Value::as_str)
                {
                    out.push(v.to_owned());
                }
                fields.values().for_each(|v| walk(v, ty, field, out));
            }
            Value::Array(items) => items.iter().for_each(|i| walk(i, ty, field, out)),
            _ => {}
        }
    }
    walk(json.get("css").unwrap_or(&Value::Null), ty, field, &mut out);
    out
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

/// The printer keeps the run at EVERY juncture the parser skips one, exactly once.
///
/// The sibling of `the_printer_keeps_every_member_of_a_mixed_run`, widened from the
/// selector-list start to the whole juncture inventory — because a skip and a preservation
/// are one change, not two. Each juncture the sweep opened moved a run out of the AST, and
/// wherever the printer regenerates that gap from parts rather than copying source, the
/// character had nowhere to ride out on: the attribute selector, the pseudo-argument parens,
/// the `,`, the `{`, an explicit combinator, a rebuilt block-child head.
///
/// ⚠️ It reads the **parser's own tables** rather than a hand-copied list of shapes. A
/// separate list is how the two halves of "a skip and a preservation are one change" drift:
/// the printer list was written by hand once, and the junctures it happened to omit —
/// `::part()`'s inter-name gap, a run stranded ahead of a comment, the whole comment-bearing
/// attribute rebuild — were exactly the ones that dropped or doubled the character while this
/// test stayed green. Adding a juncture to a table now adds it here, which is the property
/// worth having. [`PRINTER_ONLY_JUNCTURES`] carries what the tables cannot: the shapes where
/// parser and printer agree but a *second emitter* takes over.
///
/// Three properties, all of which have failed: the count is preserved, it is preserved
/// exactly (not doubled), and the output is a fixed point. The last is not implied by the
/// first two — a run re-emitted beside a separator the printer also regenerates grows by a
/// column on every pass.
#[test]
fn the_printer_keeps_the_run_at_every_juncture_exactly_once() {
    let templates = BOUNDARY_JUNCTURES
        .iter()
        .map(|(_, template, _)| *template)
        .chain(
            COMMENT_AFTER_RUN_JUNCTURES
                .iter()
                .map(|(_, template)| *template),
        )
        .chain(PRINTER_ONLY_JUNCTURES.iter().map(|(_, template)| *template));
    for template in templates {
        for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
            for run in [
                ch.to_owned(),
                format!("{ch} "),
                format!(" {ch}"),
                format!("{ch}\t{ch}"),
            ] {
                let src = component(&template.replace("{T}", &run));
                let out = tsv_svelte::format_str(&src).expect("component should format");
                assert_eq!(
                    out.matches(ch).count(),
                    run.matches(ch).count(),
                    "{label} {template} (run {run:?}): every member survives formatting, and \
                     none is emitted twice — got {out:?}"
                );
                let again = tsv_svelte::format_str(&out).expect("output should re-format");
                assert_eq!(
                    again, out,
                    "{label} {template} (run {run:?}): formatting must be a fixed point"
                );
            }
        }
    }
}

/// The junctures whose PRESERVATION has a second emitter the parser tables cannot name.
///
/// Every entry is a position the parser already covers, reached through a printer path the
/// table's own spelling does not exercise: a comment anywhere in a selector routes the whole
/// thing to `build_complex_selector_doc_with_comments`, a comment anywhere in an attribute
/// selector routes it to `build_commented_attribute_selector_text`, and `::part()` rebuilds
/// its names from spans. A juncture that is restored on one path and dropped on the other is
/// the same bug as one that is never restored, and only these spellings can see it.
const PRINTER_ONLY_JUNCTURES: [(&str, &str); 16] = [
    // `::part()` rebuilds every name from its span, so its inter-name gap has no carrier at
    // all — `join(" ")` regenerated it and deleted the run outright.
    (
        "a `::part()` inter-name gap",
        "<style>::part(a {T}b) { color: red; }</style>",
    ),
    (
        "a `::part()` inter-name gap, with a comment",
        "<style>::part(a {T}/* c */ b) { color: red; }</style>",
    ),
    // A run STRANDED ahead of a comment: the anchor's backward scan stops at the `*/` and
    // never reaches it, so the gap needs its forward sweep too.
    (
        "a stranded run in a descendant gap",
        "<style>a {T}/* c */ b { color: red; }</style>",
    ),
    (
        "a stranded run before a combinator",
        "<style>a {T}/* c */ > b { color: red; }</style>",
    ),
    (
        "a stranded run after a combinator",
        "<style>a > {T}/* c */ b { color: red; }</style>",
    ),
    (
        "a stranded run in a pseudo-argument's lead",
        "<style>:is({T}/* c */ a) { color: red; }</style>",
    ),
    (
        "a stranded run before a relative argument's combinator",
        "<style>a:has({T}/* c */ > b) { color: red; }</style>",
    ),
    // An anchorless combinator (an empty compound): its symbol is the only anchor the run has.
    (
        "an anchorless combinator's gap",
        "<style>:has({T}> > a) { color: red; }</style>",
    ),
    (
        "an anchorless combinator's gap, with a comment",
        "<style>:has({T}> /* c */ > a) { color: red; }</style>",
    ),
    // The comment-bearing attribute rebuild, one entry per interior gap: a scan whose
    // whitespace class was narrower than the parser's mislocated every part behind the run.
    (
        "a commented attribute's matcher gap",
        "<style>[a/* c */{T}='b'] { color: red; }</style>",
    ),
    (
        "a commented attribute's two-char matcher",
        "<style>[a/* c */{T}^='b'] { color: red; }</style>",
    ),
    (
        "a commented attribute's value gap",
        "<style>[a=/* c */{T}'b'] { color: red; }</style>",
    ),
    (
        "a commented attribute's flag gap",
        "<style>[a='b'/* c */{T}i] { color: red; }</style>",
    ),
    // Behind the comment, never glued to the flag: a run against the flag's own end is the
    // tracked trailing-run residue (`[a=b i<NBSP>]`), which tsv REJECTS.
    (
        "a commented attribute's flag tail",
        "<style>[a='b' i /* c */{T}] { color: red; }</style>",
    ),
    (
        "a commented attribute's `]` gap",
        "<style>[a{T}/* c */] { color: red; }</style>",
    ),
    // A block child whose head the printer REBUILDS, reached with a comment in front of it.
    (
        "a rebuilt block-child head behind a comment",
        "<style>a { /* c */ {T}color: red; }</style>",
    ),
];

/// A format must not move a name's own boundary — the one drop this file's counting cannot
/// see, because nothing is dropped.
///
/// Flush emission is safe wherever the run lands against a `,`, a `{`, a `)` or a quote: it
/// can only re-parse as the run the parser skipped. Against a **name** it glues into one
/// (`read_identifier` takes every code point at or above U+00A0 as content), so `[a<NBSP>]`
/// re-parses with the name `a<NBSP>` — a document whose AST changed under a format, and whose
/// second pass is its own fixed point, so idempotency, reparse, counting and the ledger are
/// all blind. Worse than the drop it replaced, too: the dropped spelling still SELECTED, and
/// `a<NBSP>` matches nothing.
///
/// ⚠️ **This is not one gap, and reading it as one is what let the class back in.** Every
/// claim whose left neighbour can be a name owes the separator, and the family has five:
/// the attribute selector's `[name<HERE>]`, a selector list's `,` (`a.x <NBSP>, c`), an
/// explicit combinator's symbol (`a <NBSP>> b`), a pseudo-argument list's `)`
/// (`:is(a <NBSP>)`), and every interior gap of the commented attribute rebuild
/// (`[a/* c */ <NBSP>=b]`). One rule answers all of them —
/// `Printer::name_run_separator` from a source position, `name_run_separator_after` from
/// built text — so the templates below are one per EMITTER, with the non-name left
/// neighbours (`a[b] <NBSP>,`, `* <NBSP>,`) beside them as the controls that keep the
/// separator from becoming unconditional.
#[test]
fn a_preserved_run_never_moves_a_names_boundary() {
    for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
        for template in [
            // The attribute selector's own tail, plain and comment-bearing.
            "<style>[a {T}] { color: red; }</style>",
            "<style>[a {T}/* c */] { color: red; }</style>",
            "<style>[a='b' {T}] { color: red; }</style>",
            "<style>[a='b' {T}i] { color: red; }</style>",
            "<style>[svg|a {T}] { color: red; }</style>",
            // …and the commented rebuild's interior gaps, which resume right behind the NAME.
            "<style>[a/* c */ {T}=b] { color: red; }</style>",
            "<style>[a {T}/* c */=b] { color: red; }</style>",
            "<style>[a='b' {T}/* c */ i] { color: red; }</style>",
            "<style>[a='b'/* c */ {T}i] { color: red; }</style>",
            // A selector list's `,`, one per shape the previous selector can end in — the
            // last two are the controls, where nothing can glue.
            "<style>a {T}, b { color: red; }</style>",
            "<style>a b {T}, c { color: red; }</style>",
            "<style>a.x {T}, c { color: red; }</style>",
            "<style>a::before {T}, c { color: red; }</style>",
            "<style>a {T}, b {T}, c { color: red; }</style>",
            "<style>@keyframes k { 0% {T}, 50% { color: red; } }</style>",
            "<style>a[b] {T}, c { color: red; }</style>",
            "<style>* {T}, c { color: red; }</style>",
            // An explicit combinator's symbol, which the printer emits a space AFTER — so
            // only the run's own left side is at stake.
            "<style>a {T}> b { color: red; }</style>",
            "<style>a {T}~ b { color: red; }</style>",
            "<style>a {T}+ b { color: red; }</style>",
            "<style>:is(a {T}> b) { color: red; }</style>",
            "<style>a /* c */ {T}> b { color: red; }</style>",
            // …and the anchorless one, whose left neighbour is another symbol (a control).
            "<style>:has(> {T}> a) { color: red; }</style>",
            // A pseudo-argument list's `)`, across the arms that rebuild their own contents.
            "<style>:is(a {T}) { color: red; }</style>",
            "<style>:where(a {T}) { color: red; }</style>",
            "<style>a:has(b {T}) { color: red; }</style>",
            "<style>:is(a, b {T}) { color: red; }</style>",
            "<style>::part(a {T}) { color: red; }</style>",
            "<style>::part(a {T}b) { color: red; }</style>",
        ] {
            let src = component(&template.replace("{T}", ch));
            let out = tsv_svelte::format_str(&src).expect("component should format");
            assert_eq!(
                named_nodes(&src),
                named_nodes(&out),
                "{label} {template}: the run is preserved, but the NAME it sits beside must \
                 not absorb it — got {out:?}"
            );
            let again = tsv_svelte::format_str(&out).expect("output should re-format");
            assert_eq!(
                again, out,
                "{label} {template}: the separator must be a fixed point, not a column the \
                 next pass adds to — got {out:?}"
            );
        }
    }
}

/// A rebuilt head and the claim beside it must not BOTH keep the run.
///
/// The mirror image of the test above, and the other failure mode
/// `crate::whitespace::is_boundary_only_whitespace`'s doc names: a printer that restores
/// less than the parser skipped deletes source, and one that restores more duplicates it.
/// Two positions rebuild a head from text they trim and then restore the run beside it — a
/// declaration's property→colon gap and a condition prelude's part head — and a trim whose
/// class is narrower than the claim's leaves the character inside BOTH, so the run doubles on
/// every pass (`1 → 2 → 4 → …`).
///
/// ⚠️ **Iterate the whole class, never one member.** `str::trim` is Unicode `White_Space`,
/// which agrees with the claim's class on every member but `<ZWNBSP>` — so a single-character
/// probe passes on any of the other four and the one that doubles is exactly the one a
/// hand-picked example omits. Both trims now ask `is_boundary_whitespace`
/// (`trim_property_part`, and the condition part's segment trim), which is the claim's own
/// class.
#[test]
fn a_rebuilt_head_and_its_claim_never_both_keep_the_run() {
    for (label, ch) in JS_WHITESPACE_AT_OR_ABOVE_A0 {
        for template in [
            "<style>a { color{T}: red; }</style>",
            "<style>a { --x{T}: 1; }</style>",
            "<style>@container name {T}(a: b) { c { color: red; } }</style>",
            "<style>@supports (a: b) and {T}(c: d) { e { color: red; } }</style>",
            "<style>@supports {T}(a: b) { c { color: red; } }</style>",
        ] {
            let src = component(&template.replace("{T}", ch));
            let out = tsv_svelte::format_str(&src).expect("component should format");
            assert_eq!(
                out.matches(ch).count(),
                1,
                "{label} {template}: the run belongs to the rebuilt head or to the claim \
                 beside it, never to both — got {out:?}"
            );
            let again = tsv_svelte::format_str(&out).expect("output should re-format");
            assert_eq!(
                again, out,
                "{label} {template}: a doubling run is not a fixed point — got {out:?}"
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
