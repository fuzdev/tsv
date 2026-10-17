// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A hashbang behind a leading BOM: a whole file's BOM is its encoding mark, so the `#!`
//! after it still opens the source text (`Lexer::hashbang_at`; the fixture claim is
//! `typescript/syntax/comments/hashbang_bom_prettier_divergence`). Inside a Svelte
//! `<script>` a U+FEFF is content, not an encoding mark, so no hashbang follows it there.
//! What is pinned here is what that fixture cannot carry: the BOM-led file's wire
//! positions, which no oracle reads, and the island cells: a `<script>` or template
//! expression is never a whole source text, so no hashbang opens one.

use serde_json::Value;

const BOM: char = '\u{FEFF}';

/// The wire of a whole TypeScript file.
fn ts_wire(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("the file parses");
    tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source))
}

#[test]
fn a_file_reads_a_hashbang_behind_its_bom() {
    let plain = "#!/usr/bin/env node\nlet a;\n";
    let with_bom = format!("{BOM}{plain}");
    // File coordinates, the BOM counted as acorn counts it in a file without a hashbang:
    // the statement sits one UTF-16 unit later than in the BOM-less file.
    assert_eq!(ts_wire(plain)["body"][0]["start"], 20);
    assert_eq!(ts_wire(&with_bom)["body"][0]["start"], 21);
    assert_eq!(ts_wire(&with_bom)["start"], 0);
    // The format side strips the BOM and keeps the hashbang.
    assert_eq!(
        tsv_ts::format_str(&with_bom).expect("the file formats"),
        plain
    );
    // Only one BOM is an encoding mark: a second is content, and the `#!` no hashbang.
    let arena = bumpalo::Bump::new();
    assert!(tsv_ts::parse(&format!("{BOM}{BOM}{plain}"), &arena).is_err());
}

#[test]
fn a_script_reads_no_hashbang_behind_a_bom() {
    let arena = bumpalo::Bump::new();
    for source in [
        format!("<script>{BOM}#!/x\nlet a;</script>"),
        format!("<script lang=\"ts\">{BOM}#!/x\nlet a;</script>"),
    ] {
        assert!(
            tsv_svelte::parse(&source, &arena).is_err(),
            "accepted: {source:?}"
        );
    }
}

#[test]
fn an_island_opening_on_a_hashbang_is_rejected() {
    // A hashbang opens only a whole source text. A `<script>` or a template expression is never
    // that, so a `#!` at its first byte is no hashbang, and Svelte's parser rejects it too.
    let arena = bumpalo::Bump::new();
    for source in [
        "<script>#!/x\nlet a;</script>",
        "<script lang=\"ts\">#!/x\nlet a;</script>",
        "<script module>#!/x\nlet a;</script>",
        "{#if #!x\nready}ok{/if}",
        "{@html #!x\ncontent}",
    ] {
        assert!(
            tsv_svelte::parse(source, &arena).is_err(),
            "accepted: {source:?}"
        );
    }
}
