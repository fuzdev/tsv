// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The gap between a leading HTML comment and a lifted `<script>` / `<style>`, which decides
//! whether the comment reaches that root. Svelte answers it in `1-parse/state/element.js`, by
//! walking the fragment backwards over nodes that are a `Comment` or a `Text` whose **decoded**
//! `data` is `.trim()`-empty. That is a JS `.trim()` over decoded
//! text, and tsv asked it as `str::trim` over RAW SOURCE — two independent errors that happen
//! to live in one expression, and the comment either reaches the script/style node's
//! `leadingComments` / `content.comment` or it does not.
//!
//! **Why a test rather than a fixture.** None of these inputs is a formatting fixed point —
//! prettier moves the tag onto its own line whatever fills the gap — so none can be an
//! `input.*` (F1), and the claim is wire-only. Same arrangement, and the same weakness, as
//! [css_boundary_whitespace.rs](./css_boundary_whitespace.rs): every expectation here is
//! transcribed from the live modern parser (`cargo run -p tsv_debug canonical_parse`) rather
//! than regenerated, so it would go stale silently if Svelte changed.
//!
//! ⚠️ **Not to be confused with the sanctioned anti-duplication stance**, which lives in the
//! same helper and is pinned at the bottom of this file. Svelte attaches one leading comment
//! to *every* lifted root that follows it; tsv attaches it once, to the nearest. That is a
//! cataloged `_svelte_divergence` (conformance_svelte.md §Comment Attachment Differences), and
//! a fix to the gap CLASS must not disturb it — which is why the controls below are here.

use serde_json::Value;

/// The comment attached to the document's one lifted root, whichever kind it is: a script's
/// `content.leadingComments[0].value` or the stylesheet's `content.comment.data`. Svelte
/// spells the same decision two ways, so one accessor keeps the table below flat.
fn attached_comment(src: &str) -> Option<String> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    for key in ["instance", "module"] {
        if let Some(v) = json
            .get(key)
            .and_then(|s| s.get("content"))
            .and_then(|c| c.get("leadingComments"))
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|c| c.get("value"))
            .and_then(Value::as_str)
        {
            return Some(v.to_owned());
        }
    }
    json.get("css")
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("comment"))
        .and_then(|c| c.get("data"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The three lifted roots the rule serves. All three read the same helper, so a class error is
/// three bugs and a fix is one — asserted across all of them so a per-consumer regression
/// cannot hide behind the other two.
const CONSUMERS: [(&str, &str); 3] = [
    ("instance", "<script>const a = 1;</script>"),
    ("module", "<script module>const b = 2;</script>"),
    ("css", "<style>div { color: red }</style>"),
];

/// A gap Svelte's `data.trim()` empties: the comment reaches the lifted root.
///
/// `U+00A0` is the **free null control** — it is whitespace to JS `\s` AND to Rust's
/// `White_Space`, so it passes under either class and grades nothing on its own; it is here so
/// a reader can see which witnesses do the grading. The other three each fail under exactly
/// one wrong reading: `U+FEFF` under Rust's class, `&nbsp;` under a raw-source scan, and the
/// ASCII space under neither (the plain control).
const ATTACHING_GAPS: [(&str, &str); 4] = [
    ("ASCII space (control)", " "),
    ("U+00A0 NBSP (null control - both classes)", "\u{a0}"),
    ("U+FEFF ZWNBSP (JS `\\s`, NOT Rust White_Space)", "\u{feff}"),
    ("&nbsp; entity (whitespace only once DECODED)", "&nbsp;"),
];

#[test]
fn a_js_whitespace_gap_carries_the_comment_to_the_lifted_root() {
    for (consumer, tag) in CONSUMERS {
        for (label, gap) in ATTACHING_GAPS {
            let src = format!("<!-- c -->{gap}{tag}");
            assert_eq!(
                attached_comment(&src).as_deref(),
                Some(" c "),
                "{consumer} / {label}: the gap is whitespace to Svelte, so the comment attaches"
            );
        }
    }
}

/// `U+0085` NEL is `White_Space` to Rust and **not** JS `\s`, so it is content in this gap and
/// the comment does NOT reach the root. The mirror of the `U+FEFF` case above, and the reason
/// a single witness cannot grade this: `str::trim` gets one of the two right whichever way it
/// is wrong, so a half-fix passes any test that asks only one.
#[test]
fn a_next_line_gap_is_content_and_blocks_the_attachment() {
    for (consumer, tag) in CONSUMERS {
        let src = format!("<!-- c -->{}{tag}", '\u{85}');
        assert_eq!(
            attached_comment(&src),
            None,
            "{consumer}: U+0085 is not JS `\\s`, so it is text between the comment and the root"
        );
    }
}

/// Ordinary text in the gap blocks it in every reading — the arm that must not become
/// collateral of widening the whitespace class.
#[test]
fn real_text_in_the_gap_blocks_the_attachment() {
    for (consumer, tag) in CONSUMERS {
        let src = format!("<!-- c -->x{tag}");
        assert_eq!(
            attached_comment(&src),
            None,
            "{consumer}: `x` is not whitespace"
        );
    }
}

/// ⚠️ **The sanctioned divergence, pinned as a control.** Svelte attaches one leading comment
/// to EVERY lifted root that follows it; tsv attaches it once, to the nearest, and the
/// intervening lifted tag is what stops the walk. Cataloged in conformance_svelte.md §Comment
/// Attachment Differences and pinned by
/// `svelte/script/leading_html_comment_instance_duplication_svelte_divergence`.
///
/// It lives in the same helper as the gap class above, so it is asserted here too: a "fix"
/// that reproduces Svelte's backwards walk faithfully would close the gap bugs and silently
/// reopen the duplication, and no fixture in the tree grades the `<style>` direction.
#[test]
fn a_lifted_tag_between_stops_the_walk_the_sanctioned_stance() {
    // Each document has TWO lifted roots after one comment. Canonical gives the comment to
    // both; tsv gives it to the first and leaves the second bare.
    for (label, src, first_key, second_key) in [
        (
            "module then instance",
            "<!-- c -->\n<script module>const b = 2;</script>\n<script>const a = 1;</script>",
            "module",
            "instance",
        ),
        (
            "instance then style",
            "<!-- c -->\n<script>const a = 1;</script>\n<style>div { color: red }</style>",
            "instance",
            "css",
        ),
        (
            "style then instance",
            "<!-- c -->\n<style>div { color: red }</style>\n<script>const a = 1;</script>",
            "css",
            "instance",
        ),
    ] {
        let arena = bumpalo::Bump::new();
        let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
        let json = tsv_svelte::convert_ast_json(&ast, src);
        let comment_of = |key: &str| -> Option<String> {
            let node = json.get(key)?.get("content")?;
            let from_script = node
                .get("leadingComments")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(|c| c.get("value"))
                .and_then(Value::as_str);
            from_script
                .or_else(|| node.get("comment")?.get("data")?.as_str())
                .map(str::to_owned)
        };
        assert_eq!(
            comment_of(first_key).as_deref(),
            Some(" c "),
            "{label}: the NEAREST lifted root keeps the comment"
        );
        assert_eq!(
            comment_of(second_key),
            None,
            "{label}: the second root does NOT get a copy - canonical duplicates here and tsv \
             deliberately does not"
        );
    }
}
