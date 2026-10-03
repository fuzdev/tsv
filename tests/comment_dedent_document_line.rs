// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! Which LINE the comment dedent measures: the document's own, on every island.
//!
//! `onComment` (`svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js`) dedents a
//! multi-line block comment by the `[ \t]` run its line opens with. Svelte reads that run out of
//! the string its reader handed acorn, and four readers manufacture that string — `read_script`
//! blanks the prefix, `read_pattern` wraps the binding as `(pattern = 1)`,
//! `read_type_annotation` writes `_ as ` over the colon, and a `{#snippet}` head blanks the
//! non-whitespace — so on the line a manufacture ends, Svelte measures a run the document does
//! not hold. tsv measures the document's line there too. That difference is cataloged in
//! `docs/conformance_svelte.md` §Comment Attachment Differences, and its template readers are
//! pinned against the oracle by the `<!-- prettier-ignore -->`-frozen fixture
//! `tests/fixtures/svelte/syntax/comments/head_multiline_comment_dedent_svelte_divergence`.
//!
//! **Why a test rather than a fixture.** The trigger is a comment that OPENS on the manufactured
//! line, and unfrozen formatting moves it off that line in every shape below: a `<script>`'s
//! first statement goes on a line of its own (and prettier reformats a script body through an
//! ignore directive, so no fixture can hold the `<script>` reader at all), a destructuring
//! pattern expands one property per line, and an annotation head the formatter joins back onto
//! one line no longer carries the newline the `_ as ` would swallow.
//!
//! `tests/comment_dedent_line_terminators.rs` is the sibling for the other thing this dedent
//! reads two ways: which line-terminator class each of its two steps takes.

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
/// and returns the one answer.
fn comment_values(src: &str) -> Vec<String> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the component");
    let json = tsv_debug::json::wire_value(&tsv_svelte::convert_ast_json_bytes(&ast, src));
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

/// On the line a manufacture ends, the run is the document's — once per reader, each with the
/// value Svelte gives instead noted beside it (transcribed from the live modern Svelte parser
/// via `cargo run -p tsv_debug canonical_parse`).
///
/// The rows come in both directions, because the manufactured run can be shorter or longer
/// than the document's: an indented head's tab reads to Svelte as a blanked space, so Svelte
/// strips nothing where tsv strips the tab; and a column-0 `<script>`'s eight blanked columns
/// strip eight spaces Svelte sees and tsv does not, where the document line opens with no run
/// at all.
#[test]
fn the_dedent_reads_the_documents_line_on_a_manufactured_line() {
    for (label, src, expected) in [
        // Svelte: `"\nc1 "` — the eight blanked columns of `<script>` come off.
        (
            "script prefix, tag at column 0",
            "<script>/*\n        c1 */\nlet a = 1;\n</script>\n",
            "\n        c1 ",
        ),
        // Svelte: `" a1\n\t a2 "` — the tag's tab is a blanked space.
        (
            "script prefix, indented tag",
            "\t<script>/* a1\n\t a2 */ let a;\n\t</script>\n",
            " a1\n a2 ",
        ),
        // Svelte: `"\n\t c1 "` — `read_pattern`'s prefix is blanked.
        (
            "destructuring pattern",
            "{#if c}\n\t{@const { a = /*\n\t c1 */ 1 } = expr}\n{/if}\n",
            "\n c1 ",
        ),
        // Svelte: `"\n\t c1 "` — `{#snippet}`'s prelude keeps the tab and blanks past it.
        (
            "snippet head",
            "{#if c}\n\t{#snippet s(a = /*\n\t c1 */ 1)}{/snippet}\n{/if}\n",
            "\n c1 ",
        ),
        // Svelte: `"\n\t c1 "` — `read_type_annotation`'s prefix is blanked up to its `_ as `.
        (
            "annotation, unbroken head",
            "<script lang=\"ts\"></script>\n{#if c}\n\t{@const a5: /*\n\t c1 */ T = e}\n{/if}\n",
            "\n c1 ",
        ),
        // Svelte: `" a1\n\t a2 "` — the `_ as ` swallows the `\n` before the colon, so acorn's
        // line opens back on the binding's; the document's line is the colon's own.
        (
            "annotation, newline before the colon",
            "<script lang=\"ts\">\n\tlet xs = [1];\n</script>\n{#if xs}\n\
             \t{#each xs as x\n\t: /* a1\n\t a2 */ number}{x}{/each}\n{/if}\n",
            " a1\n a2 ",
        ),
    ] {
        assert_eq!(comment_value(src), expected, "{label}");
    }
}

/// The controls: every line that is not a manufacture's last reads the same in both parsers,
/// so these are matches with the oracle, transcribed like the rows above.
///
/// - `read_expression` hands acorn the raw template, so the line acorn measured IS the
///   document's;
/// - past a manufacture's last line the string acorn was handed is the document again, so a
///   destructure or a `{#snippet}` head broken across lines takes the document's `\t\t`.
#[test]
fn the_dedent_matches_svelte_off_a_manufactured_line() {
    for (label, src, expected) in [
        (
            "raw template, `{@const}` init",
            "{#if a}\n\t{@const b = /* a1\n\t a2 */ 1}\n\t{b}\n{/if}\n",
            " a1\n a2 ",
        ),
        (
            "raw template, expression tag",
            "{#if a}\n\t{expr /* a1\n\t a2 */}\n{/if}\n",
            " a1\n a2 ",
        ),
        (
            "destructuring pattern, comment past the boundary",
            "{#if c}\n\t{@const {\n\t\ta = /*\n\t\t c1 */ 1\n\t} = e}\n{/if}\n",
            "\n c1 ",
        ),
        (
            "snippet head, comment past the boundary",
            "{#if c}\n\t{#snippet s(\n\t\ta = /*\n\t\t c1 */ 1\n\t)}{/snippet}\n{/if}\n",
            "\n c1 ",
        ),
    ] {
        assert_eq!(comment_value(src), expected, "{label}");
    }
}

/// The document's first line opens past a leading byte-order mark, because Svelte strips one
/// before it parses — so its run is the `\t` behind the BOM, not the empty run byte 0 opens.
/// Matches the oracle (Svelte: `" x\ny "`); the second row is the control, a `<script>` body
/// whose comment opens on a later line, which no BOM can reach.
#[test]
fn the_first_line_opens_past_a_leading_bom() {
    for (label, src, expected) in [
        ("expression tag", "\u{feff}\t{a /* x\n\ty */}", " x\ny "),
        (
            "script body, later line",
            "\u{feff}<script>\n\tlet a = /* x\n\ty */ 1;\n</script>\n",
            " x\ny ",
        ),
    ] {
        assert_eq!(comment_value(src), expected, "{label}");
    }
}

/// A block binding is up to TWO acorn parses — the pattern's and its `: T`'s — and each comment
/// in it takes its own line's run; neither the island nor the parse is the unit. The two lines
/// open with different runs (`\t`, then `\t\t `), so one answer for the binding leaves one of
/// the two values wrong.
#[test]
fn a_bindings_two_parses_each_read_their_own_line() {
    assert_eq!(
        comment_values(
            "<script lang=\"ts\"></script>\n{#if c}\n\t{@const { a = /*\n\t\t p1 */ 1 }: /*\n\t\t t1 */ T = e}\n{/if}\n"
        ),
        ["\n\t p1 ", "\nt1 "]
    );
}

/// The agreement `comment_values` asserts is only worth asserting where an attached copy
/// EXISTS. Without this the check would pass vacuously on a wire that stopped emitting attached
/// comments at all.
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
        let json = tsv_debug::json::wire_value(&tsv_svelte::convert_ast_json_bytes(&ast, src));
        let start = json["comments"][0]["start"].as_u64();
        assert!(
            !attached_values(&json, start).is_empty(),
            "{label}: no attached copy, so `comment_values`' agreement check is vacuous here"
        );
    }
}
