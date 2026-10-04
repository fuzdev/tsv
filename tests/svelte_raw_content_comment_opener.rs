// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `<!--` opening the content of a nested `<script>` / `<style>` or a `<textarea>` is
//! content, not an HTML comment.
//!
//! Svelte reads those three bodies raw — a nested `<script>` / `<style>` up to the first
//! literal `</script>` / `</style>`, a `<textarea>` as RCDATA up to its close — so a `<!--`
//! there opens nothing and owes no `-->`. The parser therefore never lexes the bytes behind
//! such an element's opening `>` as template: lexed that way, an unterminated `<!--` fails
//! for want of a closer, and one with a `-->` further down the document parses only by
//! courtesy of that closer — which the formatter can move away.
//!
//! The `tests/fixtures/svelte/elements/textarea_comment_opener` and
//! `nested_script_style_comment_opener_prettier_divergence` fixtures pin the wire for these
//! shapes, but several to a file — and a file's last `-->` is exactly what masks the first
//! failure. These tests hold what a fixture cannot: each opener ALONE in its document, the
//! same opener with a real comment after it, and the elements whose content IS template,
//! where the same bytes stay an unterminated comment (as they are to Svelte's parser).

use serde_json::Value;

/// The opener shapes Svelte accepts: the document, the element holding the raw content, and
/// that content.
const RAW_CONTENT_OPENERS: &[(&str, &str, &str)] = &[
    ("<div><script><!--x</script></div>", "script", "<!--x"),
    ("<div><style><!-- x</style></div>", "style", "<!-- x"),
    ("<textarea><!-- x</textarea>", "textarea", "<!-- x"),
    (
        "<div><textarea><!-- x</textarea></div>",
        "textarea",
        "<!-- x",
    ),
    ("<textarea> <!-- x</textarea>", "textarea", " <!-- x"),
    ("<textarea><!--</textarea>", "textarea", "<!--"),
    ("<textarea>x<!-- y</textarea>", "textarea", "x<!-- y"),
];

fn wire(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(source, &arena).expect("the document should parse");
    tsv_debug::json::wire_value(&tsv_svelte::convert_ast_json_bytes(&ast, source))
}

/// The first element named `name` among `nodes`, depth first.
fn find_element<'v>(nodes: &'v [Value], name: &str) -> Option<&'v Value> {
    nodes.iter().find_map(|node| {
        if node["name"].as_str() == Some(name) {
            return Some(node);
        }
        find_element(node["fragment"]["nodes"].as_array()?, name)
    })
}

/// The `type` of each root node.
fn root_types(wire: &Value) -> Vec<&str> {
    wire["fragment"]["nodes"]
        .as_array()
        .expect("Root.fragment.nodes array")
        .iter()
        .map(|node| node["type"].as_str().unwrap_or("?"))
        .collect()
}

/// Assert `element`'s children are exactly one `Text` whose raw is `content`.
fn assert_sole_text(wire: &Value, element: &str, content: &str, source: &str) {
    let root = wire["fragment"]["nodes"]
        .as_array()
        .expect("Root.fragment.nodes array");
    let node = find_element(root, element).expect("the document should hold the element");
    let children = node["fragment"]["nodes"]
        .as_array()
        .expect("element fragment.nodes array");
    let raws: Vec<_> = children
        .iter()
        .map(|child| (child["type"].as_str(), child["raw"].as_str()))
        .collect();
    assert_eq!(raws, [(Some("Text"), Some(content))], "{source:?}");
}

#[test]
fn unterminated_opener_alone_in_its_document_is_content() {
    for &(source, element, content) in RAW_CONTENT_OPENERS {
        let wire = wire(source);
        assert_sole_text(&wire, element, content, source);
        assert_eq!(root_types(&wire), ["RegularElement"], "{source:?}");
    }
}

/// With a real comment further down, the opener still ends at its own element's close — the
/// `-->` belongs to the comment that follows, and the element between them is intact.
///
/// This holds with or without the opener rule: the raw scan repositions past an over-long
/// token. It is the parity pin for that direction.
#[test]
fn opener_does_not_reach_a_later_comment_closer() {
    for &(source, element, content) in RAW_CONTENT_OPENERS {
        let source = format!("{source}<p>a</p><!-- c -->");
        let wire = wire(&source);
        assert_sole_text(&wire, element, content, &source);
        assert_eq!(
            root_types(&wire),
            ["RegularElement", "RegularElement", "Comment"],
            "{source:?}"
        );
    }
}

#[test]
fn opener_documents_format_to_themselves() {
    for &(source, ..) in RAW_CONTENT_OPENERS {
        let formatted = tsv_svelte::format_str(source)
            .unwrap_or_else(|err| panic!("{source:?} should format: {err:?}"));
        assert_eq!(formatted.trim_end(), source, "{source:?}");
    }
}

/// A `<textarea>` opener followed by a top-level `<script>` whose tag carries the `-->`: the
/// script is hoisted above the textarea, so the output puts the closer BEFORE the opener.
/// It must still parse, and format to itself.
#[test]
fn hoisted_script_carrying_the_closer_formats_to_a_fixed_point() {
    let source = "<textarea><!-- x</textarea><script !-->let a;</script>";
    let formatted = tsv_svelte::format_str(source).expect("the document should format");
    assert_eq!(
        formatted,
        "<script !-->\n\tlet a;\n</script>\n\n<textarea><!-- x</textarea>\n"
    );

    let reparsed = wire(&formatted);
    assert_sole_text(&reparsed, "textarea", "<!-- x", &formatted);
    let again = tsv_svelte::format_str(&formatted).expect("the output should format");
    assert_eq!(again, formatted);
}

/// Where the content is template, the same bytes are an unterminated comment: `<title>` and
/// `<pre>` parse their children as a fragment, and a top-level `<script>` / `<style>` hands
/// its body to the embedded parser, which refuses it.
#[test]
fn opener_in_template_or_top_level_content_still_rejects() {
    for source in [
        "<title><!-- x</title>",
        "<pre><!-- x</pre>",
        "<svelte:head><title><!-- x</title></svelte:head>",
        "<style><!-- x</style>",
        "<script><!-- x</script>",
    ] {
        let arena = bumpalo::Bump::new();
        assert!(
            tsv_svelte::parse(source, &arena).is_err(),
            "{source:?} should be rejected"
        );
    }
}

/// The shapes that parsed before the opener rule and must keep their content: an opener
/// behind an `{expr}` or a `{`, and a terminated `<!-- … -->`, which is content too.
#[test]
fn neighbouring_raw_content_is_unchanged() {
    for (source, element, content) in [
        ("<textarea><!-- x --></textarea>", "textarea", "<!-- x -->"),
        (
            "<div><script><!-- x --></script></div>",
            "script",
            "<!-- x -->",
        ),
        ("<div><script>{<!--x</script></div>", "script", "{<!--x"),
    ] {
        assert_sole_text(&wire(source), element, content, source);
    }

    let source = "<textarea>{a}<!-- x</textarea>";
    let wire = wire(source);
    let root = wire["fragment"]["nodes"].as_array().expect("root nodes");
    let children: Vec<_> =
        find_element(root, "textarea").expect("a <textarea>")["fragment"]["nodes"]
            .as_array()
            .expect("children")
            .iter()
            .map(|child| (child["type"].as_str(), child["raw"].as_str()))
            .collect();
    assert_eq!(
        children,
        [
            (Some("ExpressionTag"), None),
            (Some("Text"), Some("<!-- x"))
        ]
    );
}
