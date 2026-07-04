// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! HTML5 implicit tag closing (auto-close) in the Svelte parser.
//!
//! Svelte's parser omits closing tags for a subset of HTML elements — a `<li>`
//! auto-closes at a sibling `<li>` or a parent `</ul>`, a `<p>` at a block-level
//! sibling, `<td>`/`<th>`/`<tr>`/`<dt>`/`<dd>`/`<option>`/… at their structural
//! boundaries — matching the WHATWG optional-end-tag table (the
//! `autoclosing_children` map + `closing_tag_omitted` in
//! `../svelte/packages/svelte/src/html-tree-validation.js`). tsv previously
//! rejected all of it (`Mismatched tags`), an over-rejection of ordinary Svelte
//! markup.
//!
//! Coverage is split by what each vehicle can see. The *formatter* side — that the
//! implicit form normalizes to the explicit-close form under both tsv and prettier
//! — is pinned by the `tests/fixtures/svelte/elements/implicit_close_*` fixtures
//! (the implicit form is an `unformatted_*` variant; the explicit form is the
//! idempotent `input.svelte`). What those fixtures can NOT see is the implicit
//! form's own AST: their `expected.json` pins the *explicit* form, and the
//! auto-closed `end` offsets don't change the formatted output. So these root tests
//! pin exactly that — the implicit form's wire-AST skeleton (nesting + auto-closed
//! `end`) vs the live Svelte parser — offline, in the core suite. (The
//! `conformance:svelte-fixtures` gate checks it too, but it's periodic, needing the
//! FFI + sidecar.)
//!
//! The expected skeletons below are transcribed from the live modern Svelte
//! parser (`tsv_debug canonical_parse`) over Svelte's own test inputs.

use serde_json::Value;

/// Parse `src`, convert to the wire AST, and reduce `Root.fragment.nodes` to a
/// compact `name(start-end)[kids…]` skeleton — the exact auto-close structure the
/// oracle produces. Elements recurse through `fragment.nodes`; other nodes (Text,
/// IfBlock, …) render as `Type(start-end)` leaves. Auto-closed elements carry an
/// `end` at the byte offset of the tag that triggered the implicit close.
fn autoclose_skeleton(src: &str) -> String {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept implicit-close markup");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let nodes = json["fragment"]["nodes"]
        .as_array()
        .expect("Root.fragment.nodes array");
    reduce(nodes)
}

fn reduce(nodes: &[Value]) -> String {
    let mut parts = Vec::new();
    for node in nodes {
        let ty = node["type"].as_str().unwrap_or("?");
        let start = node["start"].as_i64().unwrap_or(-1);
        let end = node["end"].as_i64().unwrap_or(-1);
        let label = node.get("name").and_then(Value::as_str).unwrap_or(ty);
        let kids = node
            .get("fragment")
            .and_then(|f| f.get("nodes"))
            .and_then(Value::as_array);
        match kids {
            Some(kids) if !kids.is_empty() => {
                parts.push(format!("{label}({start}-{end})[{}]", reduce(kids)));
            }
            _ => parts.push(format!("{label}({start}-{end})")),
        }
    }
    parts.join(" ")
}

/// `<li>` auto-closed by a following sibling `<li>` and by the parent `</ul>`.
#[test]
fn implicitly_closed_li() {
    let src = "<ul>\n\t<li>a\n\t<li>b\n\t<li>c\n</ul>\n";
    assert_eq!(
        autoclose_skeleton(src),
        "ul(0-31)[Text(4-6) li(6-13)[Text(10-13)] li(13-20)[Text(17-20)] li(20-26)[Text(24-26)]]",
    );
}

/// A `<div>`/`<p>` implicitly closed by the parent's closing tag (`</main>`).
#[test]
fn implicitly_closed_by_parent() {
    let src = "<main><div class=\"hello\"></main>\n\n<main>\n\t<div class=\"hello\">\n\t\t<p>hello</p>\n</main>\n";
    assert_eq!(
        autoclose_skeleton(src),
        "main(0-32)[div(6-25)] Text(32-34) main(34-84)[Text(40-42) div(42-77)[Text(61-64) p(64-76)[Text(67-72)] Text(76-77)]]",
    );
}

/// A `<p>` implicitly closed by a block-level sibling (`<div>`, another `<p>`).
#[test]
fn implicitly_closed_by_sibling() {
    let src = "<div>\n\t<p class=\"hello\">\n\t\t<span></span>\n\t\t<p></p>\n</div>\n\n<div>\n\t<p class=\"hello\"><p></p>\n</div>\n";
    assert_eq!(
        autoclose_skeleton(src),
        "div(0-57)[Text(5-7) p(7-43)[Text(24-27) span(27-40) Text(40-43)] p(43-50) Text(50-51)] Text(57-59) div(59-97)[Text(64-66) p(66-83) p(83-90) Text(90-91)]",
    );
}

/// The full HTML optional-end-tag matrix: li/dt/dd/p/rt/rp/option/optgroup and the
/// table family (thead/tbody/tfoot/tr/td/th), each auto-closing on its own trigger.
#[test]
fn autoclosed_tags_matrix() {
    let src = "<ul><li><li></ul>\n<dl><dt><dd><dd></dl>\n<p><p><div></div>\n<ruby><rp><rt><rt></ruby>\n<select><option><optgroup><optgroup></select>\n<table><thead><tbody><tfoot><tbody><tr><td><th></tr><tr></table>\n";
    assert_eq!(
        autoclose_skeleton(src),
        "ul(0-17)[li(4-8) li(8-12)] Text(17-18) dl(18-39)[dt(22-26) dd(26-30) dd(30-34)] Text(39-40) p(40-43) p(43-46) div(46-57) Text(57-58) ruby(58-83)[rp(64-68) rt(68-72) rt(72-76)] Text(83-84) select(84-129)[option(92-100) optgroup(100-110) optgroup(110-120)] Text(129-130) table(130-194)[thead(137-144) tbody(144-151) tfoot(151-158) tbody(158-186)[tr(165-182)[td(169-173) th(173-177)] tr(182-186)]]",
    );
}

/// An unclosed non-void element (`<duiv>`) auto-closed by its parent's `</div>`.
#[test]
fn autoclosed_at_parent_close() {
    let src = "<script>\n\tlet activeTab = 0;\n\tlet activeHeading;\n\n\t$: console.log(activeHeading);\n</script>\n\n<div class=\"tabs\">\n\t<div class=\"tab-toggles\">\n\t\t<button class:active={activeTab === 0} on:click={() => activeTab = 0}>Tab 1</button>\n\t\t<button class:active={activeTab === 1} on:click={() => activeTab = 1}>Tab 2</button>\n\t\t<button class:active={activeTab === 2} on:click={() => activeTab = 2}>Tab 3</button>\n\t</div>\n\t<div class=\"tab-content\">\n\t\t{#if activeTab === 0}\n\t\t\t<div><h1 bind:this={activeHeading}>Tab 1</h1></div>\n\t\t{/if}\n\t\t{#if activeTab === 1}\n\t\t\t<div><h1 bind:this={activeHeading}>Tab 2</h1></div>\n\t\t{/if}\n\t\t{#if activeTab === 2}\n\t\t\t<div><h1 bind:this={activeHeading}>Tab 3</h1></div>\n\t\t{/if}\n\t</div>\n\t<duiv>\n</div>\n";
    assert_eq!(
        autoclose_skeleton(src),
        "Text(91-93) div(93-718)[Text(111-113) div(113-407)[Text(138-141) button(141-225)[Text(211-216)] Text(225-228) button(228-312)[Text(298-303)] Text(312-315) button(315-399)[Text(385-390)] Text(399-401)] Text(407-409) div(409-703)[Text(434-437) IfBlock(437-521) Text(521-524) IfBlock(524-608) Text(608-611) IfBlock(611-695) Text(695-697)] Text(703-705) duiv(705-712)[Text(711-712)]]",
    );
}
