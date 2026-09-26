// helper fns here aren't `#[test]`, so clippy.toml's allow-expect/panic-in-tests don't reach them
#![expect(clippy::expect_used, clippy::panic)]

//! The Svelte wire writer writes a `RegularElement`'s name WITHOUT a JSON escape scan, and
//! this file grades the claim that licenses it — and the escaping of every Svelte name
//! that keeps the scan — on the inputs that could break them.
//!
//! A Svelte name is not escape-free by any one grammar: an attribute-name run ends only at
//! `[\s=/>"']`, so it may hold a `\` or a non-whitespace control byte (`<div a\b>`), and a
//! component name — a name with no `:` whose lead is uppercase or that holds a `.` — runs
//! to `\s`, `/` or `>`, so it may hold a `"` too (`<A"b/>`). A name that is not
//! component-shaped, though, is admitted by the element parser only as a valid element name
//! — ASCII letters, digits, `-`, `PCENChar`s, a `!` declaration or an ASCII `prefix:local`
//! — which holds none of those bytes. That is the claim: every `RegularElement` name is
//! escape-free.
//!
//! ⚠️ **A violation is quiet on the wire** — a raw `a\b` reads back as `a` + backspace, the
//! right shape in the wrong bytes — so every `"name":` field is read off the emitted BYTES
//! and must be `serde_json`'s own spelling of its value, in both wire variants, and every
//! name the AST holds must be found among them. Debug builds additionally assert the claim
//! at the write.
//!
//! The corpus cannot stand in: over the real-code corpus no name needs an escape.

use tsv_svelte::ast::internal::{AttributeNode, Element, ElementKind, FragmentNode, Root};

/// Every `"name":` string field in the wire, read off the BYTES: each literal must be
/// exactly `serde_json`'s spelling of the value it decodes to. Returns the decoded values.
#[track_caller]
fn checked_name_fields(source: &str, text: &str) -> Vec<String> {
    const KEY: &str = "\"name\":\"";
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    for (at, _) in text.match_indices(KEY) {
        let open = at + KEY.len() - 1;
        let mut end = open + 1;
        while bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        let literal = &text[open..=end];
        let decoded: String = serde_json::from_str(literal)
            .unwrap_or_else(|e| panic!("{source:?}: {literal} is not a JSON string: {e}"));
        let canonical = serde_json::to_string(&decoded).expect("a str serializes");
        assert_eq!(
            literal, canonical,
            "{source:?}: a name field is not serde_json's spelling of its value"
        );
        out.push(decoded);
    }
    out
}

/// Whether no byte of `name` is one a JSON string escapes (`serde_json`'s set).
fn escape_free(name: &str) -> bool {
    !name.bytes().any(|b| b < 0x20 || b == b'"' || b == b'\\')
}

/// Every plain attribute's name.
fn attributes<'s>(attrs: &[AttributeNode<'_>], source: &'s str, out: &mut Vec<&'s str>) {
    for node in attrs {
        if let AttributeNode::Attribute(attr) = node {
            out.push(attr.name(source));
        }
    }
}

/// Every attribute and element name in the element tree (elements nested in elements; the
/// walk does not descend into blocks), each `RegularElement` name held to the escape-free
/// claim.
fn graded_names<'s>(nodes: &[FragmentNode<'_>], source: &'s str, out: &mut Vec<&'s str>) {
    fn element<'s>(el: &Element<'_>, source: &'s str, out: &mut Vec<&'s str>) {
        let name = el.name(source);
        assert!(
            el.kind == ElementKind::Component || escape_free(name),
            "{source:?}: the RegularElement name {name:?} needs an escape"
        );
        out.push(name);
        attributes(el.attributes, source, out);
        graded_names(el.fragment.nodes, source, out);
    }
    for node in nodes {
        match node {
            FragmentNode::Element(el) => element(el, source, out),
            FragmentNode::SpecialElement(el) => {
                attributes(el.attributes, source, out);
                graded_names(el.fragment.nodes, source, out);
            }
            _ => {}
        }
    }
}

/// The names a root holds outside its fragment: the top-level `<script>` / `<style>` heads.
fn root_names<'s>(root: &Root<'_>, source: &'s str, out: &mut Vec<&'s str>) {
    let heads = root
        .instance
        .iter()
        .chain(root.module.iter())
        .map(|s| s.attributes);
    for attrs in heads.chain(root.css.iter().map(|s| s.attributes)) {
        attributes(attrs, source, out);
    }
}

/// Parse `source`; if it parses, hold every `RegularElement` name to the claim, and require
/// both wire variants to spell every name field canonically and to carry every AST name
/// among them. Returns the AST names, or `None` when the parser rejects the source.
#[track_caller]
fn grade(source: &str) -> Option<Vec<String>> {
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena).ok()?;
    let mut names = Vec::new();
    root_names(&root, source, &mut names);
    graded_names(root.fragment.nodes, source, &mut names);
    for bytes in [
        tsv_svelte::convert_ast_json_bytes(&root, source),
        tsv_svelte::convert_ast_json_bytes_no_locations(&root, source),
    ] {
        let text = String::from_utf8(bytes)
            .unwrap_or_else(|e| panic!("wire is not UTF-8 for {source:?}: {e}"));
        let mut fields = checked_name_fields(source, &text);
        for name in &names {
            let Some(at) = fields.iter().position(|f| f == name) else {
                panic!("{source:?}: no name field reads {name:?} — {text}");
            };
            fields.swap_remove(at);
        }
    }
    Some(names.into_iter().map(str::to_owned).collect())
}

/// Every ASCII character, and the non-ASCII classes a name run distinguishes: a Latin
/// letter, a CJK one, an astral one, U+FEFF, NBSP, U+1680, NEL, ZWJ and a combining mark.
fn alphabet() -> Vec<String> {
    let mut out: Vec<String> = (0..=0x7f_u8).map(|b| char::from(b).to_string()).collect();
    for c in [
        "é", "中", "𝕏", "\u{feff}", "\u{a0}", "\u{1680}", "\u{85}", "\u{200d}", "\u{301}",
    ] {
        out.push(c.to_owned());
    }
    out
}

/// A name of every length across the window write's copy classes and past its inline
/// width, with `c` at its front, middle and end.
fn names_around(c: &str, lead: &str) -> Vec<String> {
    let mut out = vec![
        format!("{lead}{c}"),
        format!("{lead}{c}b"),
        format!("{lead}{c}{c}"),
    ];
    for len in [1, 3, 7, 15, 31, 32, 40] {
        let pad = "b".repeat(len);
        out.push(format!("{lead}{pad}{c}"));
        out.push(format!("{lead}{c}{pad}"));
    }
    out
}

/// Attribute names in every attribute reader: an element's, a component's and a special
/// element's (`read_attribute`), and a top-level `<script>` / `<style>` head's
/// (`read_static_attribute`) — bare, with a value, and led by the character itself.
#[test]
fn attribute_names_are_written_canonically() {
    let (mut parsed, mut escaping) = (0usize, 0usize);
    for c in alphabet() {
        let mut names = names_around(&c, "a");
        names.push(c.clone());
        for name in names {
            for source in [
                format!("<div {name}></div>\n"),
                format!("<div {name}=\"v\" x></div>\n"),
                format!("<div {name}={{x}}></div>\n"),
                format!("<A {name} />\n"),
                format!("<svelte:element this=\"p\" {name}></svelte:element>\n"),
                format!("<script {name}></script>\n"),
                format!("<script {name}=v></script>\n"),
                format!("<style {name}></style>\n"),
            ] {
                if let Some(ast_names) = grade(&source) {
                    parsed += 1;
                    escaping += usize::from(ast_names.iter().any(|n| !escape_free(n)));
                }
            }
        }
    }
    assert!(parsed > 8_000, "the attribute alphabet parsed ({parsed})");
    assert!(
        escaping > 1_000,
        "names that need an escape were parsed ({escaping})"
    );
}

/// Element and component names: a component name runs to `\s`, `/` or `>`, so it may hold
/// a `"`, a `\` or a control byte; an element name — plain, custom, namespaced or a
/// declaration — may not, whatever the character offered.
#[test]
fn element_names_are_written_canonically() {
    let (mut parsed, mut escaping) = (0usize, 0usize);
    for c in alphabet() {
        let mut names = names_around(&c, "A");
        names.extend(names_around(&c, "x-"));
        names.extend(names_around(&c, "a"));
        names.extend(names_around(&c, "a."));
        names.extend(names_around(&c, "svg:a"));
        names.extend(names_around(&c, "!doc"));
        names.push(format!(".{c}"));
        names.push(c.clone());
        for name in names {
            for source in [
                format!("<{name} />\n"),
                format!("<{name}></{name}>\n"),
                format!("<div><{name} a=\"1\"></{name}></div>\n"),
            ] {
                if let Some(ast_names) = grade(&source) {
                    parsed += 1;
                    escaping += usize::from(ast_names.iter().any(|n| !escape_free(n)));
                }
            }
        }
    }
    assert!(parsed > 3_000, "the element alphabet parsed ({parsed})");
    assert!(
        escaping > 100,
        "names that need an escape were parsed ({escaping})"
    );
}

/// A shorthand `{name}` attribute's name is its identifier.
#[test]
fn a_shorthand_attribute_name_is_written_canonically() {
    for name in [
        "x",
        "é",
        "$$props",
        "a\u{200d}b",
        "𝕏",
        "abcdefghijklmnopqrstuvwxyz0123456789",
    ] {
        let names = grade(&format!("<div {{{name}}}></div>\n")).expect("the shorthand parses");
        assert!(names.iter().any(|n| n == name), "{name:?} was graded");
    }
}
