// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Svelte's comment dedent measures the line of the source **acorn was handed**, which for
//! four of its readers is a string Svelte manufactured rather than the document.
//!
//! `onComment` (`svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js`) dedents a
//! multi-line block comment by the `[ \t]` run its line opens with — read out of the very
//! string that parse got. Svelte prepares that string four ways
//! (`tsv_lang::AcornPrefixText`), and each one changes the answer for a comment opened on
//! the line the preparation covers:
//!
//! - `read/script.js` blanks the whole prefix with `replace(/[^\n]/g, ' ')`, so an indented
//!   `<script>` tag reads as spaces and the author's tab is not what acorn measured;
//! - `read/context.js`'s `read_pattern` blanks it the same way, drops its first space and
//!   puts a `(` at the end (`(pattern = 1)`);
//! - `read/context.js`'s `read_type_annotation` blanks it and writes `_ as ` over the five
//!   bytes at its end;
//! - `state/tag.js`'s `{#snippet}` head blanks only the NON-whitespace (`replace(/\S/g, ' ')`),
//!   so the author's own tab survives and everything after it becomes spaces.
//!
//! Measuring the document instead strips an indent acorn never saw — or, where the region's
//! own text opens with whitespace, fails to strip one it did. Both directions are below.
//!
//! **Why a test rather than a fixture.** The `<script>` half cannot be a fixture *input*:
//! both formatters put a script's content on its own line, below the tag, and a comment on a
//! later line opens past the manufacture entirely — so the prettier fixed point F2 requires
//! is exactly the shape that has nothing to test. `<!-- prettier-ignore -->` does not rescue
//! it either; prettier reformats the script body through it. The three TEMPLATE readers *are*
//! fixturable behind that carrier and are pinned by
//! `tests/fixtures/svelte/syntax/comments/head_multiline_comment_dedent` — they stay here too,
//! because a shape that dropped them would assert the rule without ever running its null
//! control. Every expectation is transcribed from the live modern Svelte parser
//! (`cargo run -p tsv_debug canonical_parse`), never derived — a derivation got the snippet
//! row backwards, and it agreed with tsv at the one spelling (column 0) where the document's
//! run is empty and every reading coincides — so it would go stale silently if `onComment`
//! changed.
//!
//! `tests/comment_dedent_line_terminators.rs` is the same arrangement for the *other* thing
//! this dedent reads two ways — which line-terminator class each of its two steps takes.

/// The one comment's dedented wire `value` — the field `onComment` writes.
fn comment_value(src: &str) -> String {
    let mut values = comment_values(src);
    assert_eq!(values.len(), 1, "this case carries exactly one comment");
    values.remove(0)
}

/// Every comment's dedented wire `value`, in position order.
///
/// The wire carries each value in up to two places: the root `comments` array, emitted
/// outside any island's walk, and the `leadingComments` / `trailingComments` copy acorn's
/// attach put on a node. They are two emitters over one comment, so this asserts they agree
/// and returns the one answer — a disagreement is the shape a per-ISLAND (rather than
/// per-comment) dedent lookup produces, which is a real bug this suite has already had.
fn comment_values(src: &str) -> Vec<String> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the component");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let comments = json["comments"]
        .as_array()
        .expect("wire root should carry a `comments` array");
    comments
        .iter()
        .map(|comment| {
            let root = comment["value"]
                .as_str()
                .expect("a comment's `value` should be a string")
                .to_owned();
            for attached in attached_values(&json, comment["start"].as_u64()) {
                assert_eq!(
                    attached, root,
                    "an attached copy disagrees with the root `comments` entry"
                );
            }
            root
        })
        .collect()
}

/// Every attached copy of the comment at `start`, anywhere in the tree.
fn attached_values(node: &serde_json::Value, start: Option<u64>) -> Vec<String> {
    let mut found = Vec::new();
    collect_attached(node, start, &mut found);
    found
}

fn collect_attached(node: &serde_json::Value, start: Option<u64>, found: &mut Vec<String>) {
    match node {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_attached(item, start, found);
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(key.as_str(), "leadingComments" | "trailingComments") {
                    for comment in value.as_array().into_iter().flatten() {
                        // A `<script>`'s preceding HTML comment is positionless (Svelte builds
                        // it), so `start` is what identifies our own.
                        if comment["start"].as_u64() == start {
                            found.push(comment["value"].as_str().unwrap_or_default().to_owned());
                        }
                    }
                }
                collect_attached(value, start, found);
            }
        }
        _ => {}
    }
}

/// *Which source* the indentation is taken from — the axis none of the terminator
/// spellings (`tests/comment_dedent_line_terminators.rs`) reaches.
///
/// The dedent closes over the string acorn was handed, and four of Svelte's parses do not hand
/// it the document. They blank the prefix two different ways, and the difference is the point:
///
/// - `read_script`, `read_pattern` and `read_type_annotation` (`1-parse/read/{script,context}.js`)
///   use `replace(/[^\n]/g, ' ')`, so the author's tab reaches acorn as SPACES;
/// - `{#snippet}`'s head (`1-parse/state/tag.js`) uses `replace(/\S/g, ' ')`, which keeps the
///   tab and blanks only what follows it, so the run is the tab PLUS the blanked columns after
///   it — longer than anything the document holds.
///
/// So reading the document diverges in both directions: it strips a tab the oracle leaves
/// standing, and leaves spaces the oracle takes off. Both are here, and so are the two edges
/// that a width-based reading gets wrong — `read_pattern` deletes one space from its blanked
/// prefix (to keep its synthetic `(` from shifting the pattern's columns), and a run that
/// reaches the real text carries on through whatever whitespace is there.
///
/// **Why these live here and not only in a fixture.** The trigger is a multi-line block
/// comment that OPENS on the synthetic region's own line, and unfrozen formatting moves it
/// off that line in every shape below — a `{@const}` destructuring pattern expands one
/// property per line, a `<script>` puts its first statement on a line of its own, a
/// `{#snippet}` head breaks its params — so none is the fixed point F1 requires — nor is an
/// annotation whose head the formatter joins back onto one line. The template readers ride
/// the `<!-- prettier-ignore -->`-frozen fixture named in the module doc; the one spelling
/// stable unfrozen, an unbroken annotation head over a tab-indented body, is
/// `tests/fixtures/svelte/tags/const/const_annotation_comment_svelte_divergence`.
/// Every expectation here is transcribed from the live modern Svelte parser
/// (`cargo run -p tsv_debug canonical_parse`), never derived — a derivation got the snippet
/// row backwards, and it agreed with tsv at the one spelling (column 0) where the document's
/// run is empty and every reading coincides.
#[test]
fn the_dedent_reads_the_source_acorn_was_handed() {
    for (label, src, expected) in [
        // `read_pattern`: `<blanked>(… = 1)`. The comment opens on the pattern's line, whose
        // one-tab prefix acorn sees as spaces, so the tab in the body survives.
        (
            "destructuring pattern",
            "{#if c}\n\t{@const { a = /*\n\t c1 */ 1 } = expr}\n{/if}\n",
            "\n\t c1 ",
        ),
        // The same pattern on the document's FIRST line, where the one space `read_pattern`
        // deletes falls inside this line's own run and shortens it by one: fourteen columns
        // come off, not the fifteen the document counts to the pattern's `{`.
        (
            "destructuring pattern, first line",
            "{#if c}{@const { a = /*\n              c1 */ 1 } = e}{/if}\n",
            "\nc1 ",
        ),
        // `read_script`: `<blanked><body>`. Here the line opens at column 0 with no `[ \t]` at
        // all, so reading the document strips NOTHING — while acorn sees the eight blanked
        // columns of `<script>` and takes them off a body line that begins with eight spaces.
        (
            "script prefix",
            "<script>/*\n        c1 */\nlet a = 1;\n</script>\n",
            "\nc1 ",
        ),
        // The same shape one space short of the blanked run: nothing matches on either side, so
        // this is the null control for the direction above.
        (
            "script prefix, short by one",
            "<script>/*\n       c1 */\nlet a = 1;\n</script>\n",
            "\n       c1 ",
        ),
        // Past the blanked prefix the synthetic source IS the document, so the run carries on
        // through the two real spaces after `<script>` — ten columns, not eight.
        (
            "script prefix, run continues",
            "<script>  /*\n          c1 */\nlet a = 1;\n</script>\n",
            "\nc1 ",
        ),
        // `{#snippet}`'s `\S` prelude keeps the author's tab and blanks the columns after it,
        // so the run is `\t` + twelve spaces — which matches nothing, where the document's bare
        // `\t` matches and would strip.
        (
            "snippet head, indented",
            "{#if c}\n\t{#snippet s(a = /*\n\t c1 */ 1)}{/snippet}\n{/if}\n",
            "\n\t c1 ",
        ),
        // The same head at column 0, where the document's run is empty and every reading
        // coincides — the vacuity control for the row above.
        (
            "snippet head, column 0",
            "{#snippet s(a = /*\n\t c1 */ 1)}{/snippet}\n",
            "\n\t c1 ",
        ),
        // `read_type_annotation` enters five bytes BEHIND the type, splicing its `_ as ` OVER
        // those bytes rather than between them — so the dedent's line walk, which runs over
        // the synthetic string, cannot see a `\n` inside that window. All four rows carry the
        // same tab-indented body, and the last two are the pair that shows the run is really
        // measured: the head is unbroken, so acorn sees the seven blanked columns ahead of the
        // splice, and a body line carrying exactly those seven loses them.
        //
        // (Each needs the `lang="ts"` script: `parser.ts` is what makes canonical read a
        // `{@const}` annotation at all, and without it there is no oracle for these rows.)
        (
            "annotation, line opens ON the splice",
            "<script lang=\"ts\"></script>\n{#if c}\n\t{@const\n\t a5: /*\n\t c1 */ T = e}\n{/if}\n",
            "\n\t c1 ",
        ),
        (
            "annotation, line opens INSIDE the splice",
            "<script lang=\"ts\"></script>\n{#if c}\n\t{@const a5\n\t: /*\n\t c1 */ T = e}\n{/if}\n",
            "\n\t c1 ",
        ),
        (
            "annotation, unbroken head, body on the blanked run",
            "<script lang=\"ts\"></script>\n{#if c}\n\t{@const a5: /*\n       c1 */ T = e}\n{/if}\n",
            "\nc1 ",
        ),
        (
            "annotation, unbroken head, one column short",
            "<script lang=\"ts\"></script>\n{#if c}\n\t{@const a5: /*\n      c1 */ T = e}\n{/if}\n",
            "\n      c1 ",
        ),
    ] {
        assert_eq!(comment_value(src), expected, "{label}");
    }
}

/// The run is BUILT rather than measured, so it has to be built in the units the blanked
/// source is spelled in — and both of them are JavaScript's, not Rust's.
///
/// - `{#snippet}`'s prelude is `replace(/\S/g, ' ')`, and `\S` is the complement of JS `\s`:
///   it does **not** match U+FEFF (which therefore survives and ends the run) and it **does**
///   match U+0085 (which therefore blanks to a space and continues it). Rust's
///   `char::is_whitespace` is the Unicode `White_Space` property, which disagrees on exactly
///   those two — [`tsv_lang::is_js_whitespace`] is the class Svelte spells.
/// - `String.prototype.replace` walks UTF-16 code units, so an astral character in the blanked
///   prefix becomes **two** spaces, not one. Both blanking flavors are affected, and the run
///   they hand the dedent is a length, so one column off is a whole strip won or lost.
///
/// Each row is paired with an ASCII control of the same shape at the same width, which pins
/// that the row's answer comes from the character class and not from the spacing.
#[test]
fn the_blanked_run_is_built_in_javascripts_units() {
    for (label, src, expected) in [
        // `\S` blanking, U+FEFF: JS `\s` HOLDS it, so it survives the prelude and stops the
        // run at the six columns of `<span>` — which is what the second line carries.
        (
            "snippet head, U+FEFF in the prefix",
            "<span>\u{feff}</span>{#snippet t(a = /*\n      c1 */ 1)}{/snippet}\n",
            "\nc1 ",
        ),
        // U+0085, the other half of the disagreement: JS `\s` does NOT hold it, so `\S` blanks
        // it and the run runs on past six columns, matching nothing.
        (
            "snippet head, U+0085 in the prefix",
            "<span>\u{85}</span>{#snippet t(a = /*\n      c1 */ 1)}{/snippet}\n",
            "\n      c1 ",
        ),
        // The control: an ordinary character in the same slot blanks and the run runs on.
        (
            "snippet head, ASCII control",
            "<span>x</span>{#snippet t(a = /*\n      c1 */ 1)}{/snippet}\n",
            "\n      c1 ",
        ),
        // `[^\n]` blanking, an astral character: two code units, so two spaces. Sixteen
        // columns come off, not the fifteen a per-`char` count would take.
        (
            "pattern, astral character in the prefix",
            "{#if c}\u{1f600}{@const { a = /*\n                c1 */ 1 } = e}{/if}\n",
            "\nc1 ",
        ),
        // The control: two ASCII characters in the same slot, same width, same answer.
        (
            "pattern, two-character ASCII control",
            "{#if c}xy{@const { a = /*\n                c1 */ 1 } = e}{/if}\n",
            "\nc1 ",
        ),
        // The two units MEET when the astral character IS the blank `read_pattern` deletes —
        // the document's first non-`\n` byte. It blanks to two spaces and the deletion takes
        // one of them, so the character contributes ONE column: nine come off `😀{@const `,
        // not the ten a width would give nor the eight a per-`char` count would.
        (
            "pattern, astral character AT the deleted blank",
            "\u{1f600}{@const { a = /*\n         c1 */ 1 } = e}\n",
            "\nc1 ",
        ),
        // One column wider: nine still come off, so a single space rides out. A per-`char`
        // count would take eight and leave two — the row above and this one bracket it.
        (
            "pattern, astral at the deleted blank, one column over",
            "\u{1f600}{@const { a = /*\n          c1 */ 1 } = e}\n",
            "\n c1 ",
        ),
        // The controls: two ASCII characters in that same slot are the same two code units,
        // so the deletion lands the same way and both answers repeat.
        (
            "pattern, two-character ASCII control at the deleted blank",
            "xy{@const { a = /*\n         c1 */ 1 } = e}\n",
            "\nc1 ",
        ),
        (
            "pattern, two-character ASCII control, one column over",
            "xy{@const { a = /*\n          c1 */ 1 } = e}\n",
            "\n c1 ",
        ),
    ] {
        assert_eq!(comment_value(src), expected, "{label}");
    }
}

/// The null control for every row above: once the comment's line opens in REAL text, the
/// synthetic source IS the document there, so the document's own run is what acorn measured
/// and the blanked prefix must not be consulted at all.
///
/// `read_script`'s side of this is already covered — the three terminator tests above all
/// put their comment on a body line — so what is here is the two the region table resolves
/// by a different rule: `read_pattern`, whose synthetic `(` sits at the boundary, and
/// `{#snippet}`'s head. Both would over-dedent if the prefix were read anyway (the pattern's
/// run is nine columns, the snippet's twelve, against the two tabs and one tab the document
/// actually opens these lines with).
///
/// Transcribed from the live modern Svelte parser, like every row above.
#[test]
fn the_dedent_reads_the_document_once_the_line_opens_in_real_text() {
    for (label, src, expected) in [
        // `read_pattern`: the destructure is broken across lines, so the comment opens on a
        // line well past the pattern's `{`. The document's `\t\t` is what comes off.
        (
            "destructuring pattern, comment past the boundary",
            "{#if c}\n\t{@const {\n\t\ta = /*\n\t\t c1 */ 1\n\t} = e}\n{/if}\n",
            "\n c1 ",
        ),
        // `{#snippet}`: same shape, past the params' `(`.
        (
            "snippet head, comment past the boundary",
            "{#if c}\n\t{#snippet s(\n\t\ta = /*\n\t\t c1 */ 1\n\t)}{/snippet}\n{/if}\n",
            "\n c1 ",
        ),
    ] {
        assert_eq!(comment_value(src), expected, "{label}");
    }
}

/// An island is NOT the unit the dedent is asked about: a block binding's is up to **two**
/// acorn parses — `read_pattern`'s over the destructure, and `read_type_annotation`'s over
/// the trailing `: T` — each blanking a different span, so one answer for the whole island
/// is wrong for whichever half it did not come from.
///
/// This is the discriminating shape, and building one takes care: the obvious two-comment
/// case has both runs strip NOTHING, so it passes under a per-island lookup too. Here the
/// pattern's blanked prefix is **eight** columns (to the `{` on line 2, less the one blank
/// `read_pattern` deletes) and the annotation's is **thirteen** (to five bytes behind the
/// `:` on line 3), and each comment's body line carries exactly its own — so a single answer
/// for the island leaves one of the two standing.
#[test]
fn an_islands_two_parses_are_resolved_separately() {
    assert_eq!(
        comment_values(
            "<script lang=\"ts\"></script>\n{@const { a = /*\n        p1 */ 1 }: /*\n             t1 */ T = e}\n"
        ),
        ["\np1 ", "\nt1 "]
    );
}

/// The agreement `comment_value` asserts is only worth asserting where an attached copy
/// EXISTS — a comment in a `{@const}` head or a `<script>` gets one, a `{#snippet}`
/// parameter default does not (nothing in that island opens before it). Without this the
/// check would pass vacuously on a wire that stopped emitting attached comments at all.
#[test]
fn the_attached_copy_is_actually_reached() {
    for (label, src) in [
        (
            "script",
            "<script>/*\n        c1 */\nlet a = 1;\n</script>\n",
        ),
        (
            "annotation",
            "<script lang=\"ts\"></script>\n{#if c}\n\t{@const a5: /*\n\t c1 */ T = e}\n{/if}\n",
        ),
        (
            "destructuring pattern",
            "{#if c}\n\t{@const { a = /*\n\t c1 */ 1 } = expr}\n{/if}\n",
        ),
    ] {
        let arena = bumpalo::Bump::new();
        let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the component");
        let json = tsv_svelte::convert_ast_json(&ast, src);
        let start = json["comments"][0]["start"].as_u64();
        assert!(
            !attached_values(&json, start).is_empty(),
            "{label}: no attached copy, so `comment_value`'s agreement check is vacuous here"
        );
    }
}

/// `read_script` blanks everything ahead of the content, so an **indented** `<script>` tag is
/// a run of spaces to acorn and the author's tab survives into the `value`.
///
/// The second case is the null control on the same reader: with the tag at column 0 there is
/// no document indentation to strip either, so both readings agree and the shape proves
/// nothing on its own.
#[test]
fn a_script_tags_own_indentation_is_blanked_out() {
    assert_eq!(
        comment_value("\t<script>/* a1\n\t a2 */ let a;\n\t</script>\n"),
        " a1\n\t a2 "
    );
    assert_eq!(
        comment_value("<script>/* a1\n\t a2 */ let a;\n</script>\n"),
        " a1\n\t a2 "
    );
}

/// The blanking is `String.replace`, which substitutes one space per **UTF-16 code unit** —
/// so a prefix holding a non-ASCII character becomes a run SHORTER than its byte span, and a
/// byte count over-dedents by one space per extra byte.
///
/// `<p>café</p><script>` is 19 code units over 20 bytes; `<p>𝔞</p><script>` is 17 over 19,
/// since an astral character is a surrogate PAIR to a `u`-less regex. Both content lines
/// below carry exactly the code-unit count, so a byte count strips nothing at all.
///
/// The null control is every other case in this file: all-ASCII, where the two counts agree
/// — which is why no fixture and no corpus can reach this. The class's other UTF-16 edges —
/// JS `\s` vs Rust `White_Space`, and the astral character AT `read_pattern`'s deleted
/// blank — are `the_blanked_run_is_built_in_javascripts_units` above.
#[test]
fn a_blanked_prefix_is_one_space_per_code_unit_not_per_byte() {
    assert_eq!(
        comment_value(&format!(
            "<p>café</p><script>/* a1\n{}a2 */ let a;\n</script>\n",
            " ".repeat(19)
        )),
        " a1\na2 "
    );
    assert_eq!(
        comment_value(&format!(
            "<p>\u{1d51e}</p><script>/* a1\n{}a2 */ let a;\n</script>\n",
            " ".repeat(17)
        )),
        " a1\na2 "
    );
}

/// The `{#snippet}` prelude blanks per code unit for the same reason, one reader over: a
/// Unicode snippet name is fewer spaces than it is bytes.
#[test]
fn a_snippet_prelude_blanks_per_code_unit_too() {
    // `{#snippet café` — 14 code units over 15 bytes.
    assert_eq!(
        comment_value(&format!(
            "{{#snippet café(b = /* a1\n{}a2 */ 1)}}{{b}}{{/snippet}}\n",
            " ".repeat(14)
        )),
        " a1\na2 "
    );
}

/// `read_type_annotation` blanks the prefix and writes `_ as ` over its last five bytes, so
/// the run acorn measured is spaces up to the insert — never the `\t` the line opens with.
#[test]
fn a_block_annotations_line_is_the_as_inserts_own() {
    assert_eq!(
        comment_value(
            "<script lang=\"ts\">\n\tlet xs = [1];\n</script>\n\
             {#if xs}\n\t{#each xs as x: /* a1\n\t a2 */ number}{x}{/each}\n{/if}\n"
        ),
        " a1\n\t a2 "
    );
}

/// The `_ as ` insert **overwrites** the five bytes it covers, so a newline the author wrote
/// between a binding and its colon is gone before acorn ever sees it — and the comment's line
/// then opens back on the *binding's* line, whose indentation is what comes off.
///
/// The two spellings below put the newline at each end of that five-byte window. The glued
/// spelling above is the null control: with the colon against the binding there is no newline
/// for the insert to swallow, and the two line starts agree.
///
/// This is the same five bytes that make an annotation's own line SEED non-identity
/// (`tests/acorn_loc_line_terminators.rs`), one question over.
#[test]
fn the_as_insert_swallows_a_newline_before_the_colon() {
    const HEAD: &str = "<script lang=\"ts\">\n\tlet xs = [1];\n</script>\n{#if xs}\n";
    assert_eq!(
        comment_value(&format!(
            "{HEAD}\t{{#each xs as\n\t x: /* a1\n\t a2 */ number}}{{x}}{{/each}}\n{{/if}}\n"
        )),
        " a1\n\t a2 "
    );
    assert_eq!(
        comment_value(&format!(
            "{HEAD}\t{{#each xs as x\n\t: /* a1\n\t a2 */ number}}{{x}}{{/each}}\n{{/if}}\n"
        )),
        " a1\n\t a2 "
    );
}

/// The null control the four share: `read_expression` hands acorn the raw template, so the
/// line acorn measured IS the document's and the tab comes off. Every case above is this same
/// authoring one reader over — without it they assert nothing about the manufacture, only
/// that some tab survived.
#[test]
fn a_raw_template_island_takes_the_documents_own_indentation() {
    assert_eq!(
        comment_value("{#if a}\n\t{@const b = /* a1\n\t a2 */ 1}\n\t{b}\n{/if}\n"),
        " a1\n a2 "
    );
    assert_eq!(
        comment_value("{#if a}\n\t{expr /* a1\n\t a2 */}\n{/if}\n"),
        " a1\n a2 "
    );
}
