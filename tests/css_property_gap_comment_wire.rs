// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A comment in a declaration's **property→colon gap** is one comment however it
//! is spaced: `color/* c */: blue` and `color /* c */ : blue` differ only in
//! whitespace, so tsv's wire AST reads them the same — one `Declaration` with
//! `property: "color"`, the colon leaking into the `value` (Svelte's quirk, which
//! tsv replicates), and one `CSSComment` on the stylesheet's flat `comments`
//! array.
//!
//! `parseCss` reads the two spellings differently. `read_declaration` takes the
//! property with `read_until(REGEX_WHITESPACE_OR_COLON)`, which is comment-BLIND:
//! on the glued spelling the first whitespace sits _inside_ the comment, so the
//! property becomes `"color/*"`, the tail `"c */: blue"` leaks into the value, and
//! the comment is captured **nowhere** — Svelte's stylesheet `comments` array
//! comes back empty. CSS Syntax 3 §4.3.2 (`consume comments` returns nothing) puts
//! tsv on the other side of that: a comment yields no token, so it cannot end a
//! property name. See `docs/conformance_svelte.md` §CSS Parser Corrections
//! (corpus-enforced) — Declaration tokenization garbage.
//!
//! Not a fixture: the glued spelling is **not a tsv fixed point** — the formatter
//! normalizes it to the spaced one (`css/tokens/comments/`
//! `in_property_value_before_colon_prettier_divergence` pins that, and
//! `tests/css_property_comment_colon_idempotent.rs` extends it across value
//! kinds), so it can only live in an `unformatted_*` variant, which makes no AST
//! claim. The corpus differential
//! (`deno task corpus:compare:parse --all`, over
//! `prettier/tests/format/css/comments/declaration.css`) is the standing gate, and
//! its `css_declaration_tokenization` matcher absorbs the whole shifted `comments`
//! array of such a document — so the two claims underneath it, tsv's reading and
//! the oracle's, are pinned here instead.

const GLUED: &str = "a {\n\tcolor/* c */: blue;\n}\n";
const SPACED: &str = "a {\n\tcolor /* c */ : blue;\n}\n";

/// The wire JSON tsv's CLI/FFI/WASM parse surfaces all emit.
fn wire(source: &str) -> serde_json::Value {
    let arena = bumpalo::Bump::new();
    let stylesheet = tsv_css::parse(source, &arena).expect("parse failed");
    tsv_css::convert_ast_json(&stylesheet, source)
}

/// `(property, value)` of the stylesheet's single declaration.
fn declaration(wire: &serde_json::Value) -> (String, String) {
    let decl = &wire["children"][0]["block"]["children"][0];
    assert_eq!(decl["type"], "Declaration", "expected one declaration");
    (
        decl["property"].as_str().expect("property").to_owned(),
        decl["value"].as_str().expect("value").to_owned(),
    )
}

/// `(value, start, end)` of each comment on the stylesheet's flat `comments` array.
fn comments(wire: &serde_json::Value) -> Vec<(String, u64, u64)> {
    wire["comments"]
        .as_array()
        .expect("comments array")
        .iter()
        .map(|c| {
            (
                c["value"].as_str().expect("comment value").to_owned(),
                c["start"].as_u64().expect("comment start"),
                c["end"].as_u64().expect("comment end"),
            )
        })
        .collect()
}

/// tsv reads both spellings as the same declaration, and keeps the comment in both
/// — the spacing moves only the comment's own offsets.
#[test]
fn both_spellings_read_the_same() {
    let glued = wire(GLUED);
    let spaced = wire(SPACED);

    assert_eq!(
        declaration(&glued),
        ("color".to_owned(), ": blue".to_owned()),
        "glued: the comment ends no property name; the colon leaks into the value \
         (the replicated Svelte quirk)"
    );
    assert_eq!(
        declaration(&spaced),
        declaration(&glued),
        "the two spellings differ only in whitespace, so the declaration is the same"
    );

    assert_eq!(
        comments(&glued),
        vec![(" c ".to_owned(), 10, 17)],
        "glued: the property-gap comment is on the stylesheet's comments array"
    );
    assert_eq!(
        comments(&spaced),
        vec![(" c ".to_owned(), 11, 18)],
        "spaced: the same comment, one column later"
    );
}

/// Live oracle: `parseCss` agrees on the spaced spelling and produces the
/// documented garbage on the glued one — the divergence the corpus matcher
/// sanctions. If Svelte makes its property scan comment-opaque, this fails and the
/// catalog entry is due for retirement (the run also needs `deno`).
#[tokio::test]
async fn sidecar_parse_css_diverges_on_the_glued_spelling() {
    let spaced = tsv_debug::deno::parse_css(SPACED)
        .await
        .expect("parseCss failed");
    assert_eq!(
        declaration(&spaced),
        declaration(&wire(SPACED)),
        "spaced: tsv replicates parseCss exactly"
    );
    assert_eq!(
        comments(&spaced),
        comments(&wire(SPACED)),
        "spaced: parseCss captures the comment too"
    );

    let glued = tsv_debug::deno::parse_css(GLUED)
        .await
        .expect("parseCss failed");
    assert_eq!(
        declaration(&glued),
        ("color/*".to_owned(), "c */: blue".to_owned()),
        "glued: read_until stops at the whitespace INSIDE the comment"
    );
    assert!(
        comments(&glued).is_empty(),
        "glued: the comment is swallowed into the property token and captured nowhere — \
         every later comment in the file is then one index behind tsv's"
    );
}
