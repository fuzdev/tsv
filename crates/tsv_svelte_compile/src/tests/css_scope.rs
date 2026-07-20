//! CSS scoping: selector matching against the element census.

use super::support::*;
use crate::*;

#[test]
fn compile_css_type_selector_synthesizes_class() {
    // A bare `<div>` scoped by a type selector gains a synthetic
    // `class="svelte-tsvhash"` (no class markup of its own), and the type selector
    // splices the hash after the tag name.
    let out = compile(
        "<div>x</div>\n<style>div{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("type selector compiles");
    assert!(
        out.js.contains(r#"<div class="svelte-tsvhash">x</div>"#),
        "synthetic scoped class: {}",
        out.js
    );
    assert_eq!(out.css.as_deref(), Some("div.svelte-tsvhash{ color: red }"));
}

#[test]
fn compile_css_type_selector_extends_existing_class() {
    // A type-scoped element with an authored static `class` appends the hash to
    // the existing value (the element is scoped by the type, not the class token).
    let js = compile_js("<div class=\"a\">x</div>\n<style>div{ color: red }</style>");
    assert!(
        js.contains(r#"<div class="a svelte-tsvhash">x</div>"#),
        "{js}"
    );
}

#[test]
fn compile_css_id_and_attribute_selectors() {
    // Id selector: synthetic class after the authored `id` attribute.
    let js = compile_js("<div id=\"foo\">y</div>\n<style>#foo{ color: red }</style>");
    assert!(
        js.contains(r#"<div id="foo" class="svelte-tsvhash">y</div>"#),
        "{js}"
    );
    // Attribute presence selector matches any value (here static).
    let js = compile_js("<p data-x=\"1\">y</p>\n<style>[data-x]{ color: red }</style>");
    assert!(
        js.contains(r#"<p data-x="1" class="svelte-tsvhash">y</p>"#),
        "{js}"
    );
    // Attribute value + explicit `i` flag matches case-insensitively.
    let js = compile_js("<p data-x=\"BAR\">y</p>\n<style>[data-x=\"bar\" i]{ color: red }</style>");
    assert!(
        js.contains(r#"<p data-x="BAR" class="svelte-tsvhash">y</p>"#),
        "{js}"
    );
}

#[test]
fn compile_css_universal_replaces_span() {
    // A bare `*` is REPLACED by the hash class (not appended).
    let out = compile(
        "<div>x</div>\n<style>*{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("universal compiles");
    assert_eq!(out.css.as_deref(), Some(".svelte-tsvhash{ color: red }"));
    // `*.c` appends on `.c` (only a bare trailing `*` replaces).
    let out = compile(
        "<div class=\"c\">x</div>\n<style>*.c{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("universal compound compiles");
    assert_eq!(out.css.as_deref(), Some("*.c.svelte-tsvhash{ color: red }"));
}

#[test]
fn compile_css_compound_needs_same_element() {
    // `.a.b` matches an element carrying BOTH classes.
    let out = compile(
        "<div class=\"a b\">x</div>\n<style>.a.b{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("same-element compound compiles");
    assert!(
        out.js.contains(r#"class="a b svelte-tsvhash""#),
        "{}",
        out.js
    );
    // `.a` and `.b` on DIFFERENT elements — no element carries both, so the
    // compound matches nothing and refuses (the oracle would comment-wrap it).
    assert_unsupported(
        "<div class=\"a\"><span class=\"b\">x</span></div>\n<style>.a.b{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_open_whitelist_on_details() {
    // `[open]` on `<details>` matches unconditionally (no `open` attribute needed).
    let js = compile_js("<details>x</details>\n<style>[open]{ color: red }</style>");
    assert!(
        js.contains(r#"<details class="svelte-tsvhash">x</details>"#),
        "{js}"
    );
}

#[test]
fn compile_css_type_matching_no_element_refuses() {
    // A type selector for an element that isn't present matches nothing → refuse.
    assert_unsupported(
        "<div>x</div>\n<style>span{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_combinator_selectors() {
    // Descendant: both compounds scope (each matched element gains the hash), the
    // first bump is a plain class, the second a zero-specificity `:where(...)`.
    let out = compile(
        "<div><p>hi</p></div>\n<style>div p{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("descendant compiles");
    assert!(
        out.js
            .contains(r#"<div class="svelte-tsvhash"><p class="svelte-tsvhash">hi</p></div>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("div.svelte-tsvhash p:where(.svelte-tsvhash){ color: red }")
    );
    // Child `>`, next-sibling `+`, subsequent-sibling `~` all splice the same way.
    assert_eq!(
        compile_css("<div><p>hi</p></div>\n<style>div > p{ color: red }</style>"),
        "div.svelte-tsvhash > p:where(.svelte-tsvhash){ color: red }"
    );
    assert_eq!(
        compile_css("<a></a><b></b>\n<style>a + b{ color: red }</style>"),
        "a.svelte-tsvhash + b:where(.svelte-tsvhash){ color: red }"
    );
    assert_eq!(
        compile_css("<a></a><b></b>\n<style>a ~ b{ color: red }</style>"),
        "a.svelte-tsvhash ~ b:where(.svelte-tsvhash){ color: red }"
    );
}

#[test]
fn compile_css_combinator_block_descent_and_each_wrap() {
    // A preceding sibling reached through a `{#if}` block (block descent) still
    // matches `a + b`.
    assert_eq!(
        compile_css("{#if x}<a></a>{/if}<b></b>\n<style>a + b{ color: red }</style>"),
        "a.svelte-tsvhash + b:where(.svelte-tsvhash){ color: red }"
    );
    // The `{#each}` self-adjacency wrap-around: a later-in-source sibling is a
    // possible runtime preceding sibling.
    assert_eq!(
        compile_css("{#each xs as x}<b></b><a></a>{/each}\n<style>a ~ b{ color: red }</style>"),
        "a.svelte-tsvhash ~ b:where(.svelte-tsvhash){ color: red }"
    );
}

#[test]
fn compile_css_combinator_no_match_refuses() {
    // A combinator chain that matches no element is pruned by the oracle — tsv
    // refuses (no `<span>` for `span + b`).
    assert_unsupported(
        "<a></a><b></b>\n<style>span + b{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_global_leading_trailing_and_bare() {
    // Leading `:global(.x) .y`: `.x` is global (no hash, wrapper stripped), `.y`
    // scopes (the first bump, plain class). The `.y` element gains the class.
    let out = compile(
        "<div class=\"y\">hi</div>\n<style>:global(.x) .y{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("leading :global compiles");
    assert!(out.js.contains(r#"class="y svelte-tsvhash""#), "{}", out.js);
    assert_eq!(
        out.css.as_deref(),
        Some(".x .y.svelte-tsvhash{ color: red }")
    );
    // Trailing `.a :global(.x)`: truncate drops `:global(.x)` from matching, but its
    // wrapper still strips in output; `.a` scopes.
    assert_eq!(
        compile_css("<div class=\"a\">hi</div>\n<style>.a :global(.x){ color: red }</style>"),
        ".a.svelte-tsvhash .x{ color: red }"
    );
    // A fully-global `:global(.x)` is never pruned and scopes no element.
    assert_eq!(
        compile_css("<div>hi</div>\n<style>:global(.x){ color: red }</style>"),
        ".x{ color: red }"
    );
    // A bare `:global` combinator: `:global` (and the preceding space) strips.
    assert_eq!(
        compile_css(
            "<div><span class=\"x\">hi</span></div>\n<style>div :global.x{ color: red }</style>"
        ),
        "div.svelte-tsvhash.x{ color: red }"
    );
}

#[test]
fn compile_css_specificity_bump_resets_per_comma() {
    // Bump state resets per comma `ComplexSelector`: the `.a` after the comma gets a
    // plain class again, not `:where(...)`.
    assert_eq!(
        compile_css("<div><p class=\"a\">hi</p></div>\n<style>div p, .a{ color: red }</style>"),
        "div.svelte-tsvhash p:where(.svelte-tsvhash), .a.svelte-tsvhash{ color: red }"
    );
}

#[test]
fn compile_css_refused_selector_shapes() {
    // The refuse-list held after slice 5: the `||` column combinator, the logical/
    // relational pseudos (`:is`/`:where`/`:has`/`:not`), `:root`, and bare
    // pseudo-only compounds. (The four real combinators and basic `:global` now
    // compile — see `compile_css_combinator_selectors` / `compile_css_global_*`.)
    assert_unsupported(
        "<div>x</div>\n<style>div || p{ color: red }</style>",
        "combinator",
    );
    for selector in [
        ":is(.a, .b)",
        ":where(.a)",
        ":has(.a)",
        ":not(.a)",
        ":root",
        ":hover",
    ] {
        assert_unsupported(
            &format!("<div>x</div>\n<style>{selector}{{ color: red }}</style>"),
            "unsupported css selector",
        );
    }
    // A `:global{}` global block stays refused — it is a nested rule, so it lands on
    // the nested-rule guard (global blocks are a deferred slice either way).
    assert_unsupported(
        "<div>x</div>\n<style>:global { .x { color: red } }</style>",
        "nested css rule",
    );
}

#[test]
fn compile_css_dynamic_attribute_value_match_refuses() {
    // A VALUED attribute selector matched against a same-named dynamic attribute
    // value the oracle would `get_possible_values`-enumerate (here an all-literal
    // ternary) is not ported — refuse rather than risk a false match.
    assert_unsupported(
        "<script>let c = $state(true);</script>\n<p data-x={c ? 'a' : 'b'}>y</p>\n<style>[data-x=\"z\"]{ color: red }</style>",
        "dynamic attribute value",
    );
}

#[test]
fn compile_css_class_split_matches_js_whitespace() {
    // The `~=` class token split must match JS `/\s/` exactly, not Rust's
    // `char::is_whitespace`. BOM (U+FEFF) is JS whitespace (not Rust's), so it
    // splits the value → `.foo` matches and the element scopes.
    let js =
        compile_js("<div class=\"foo\u{feff}bar\">x</div>\n<style>.foo { color: red }</style>");
    assert!(
        js.contains("class=\"foo\u{feff}bar svelte-tsvhash\""),
        "BOM must split the class token (JS \\s): {js:?}"
    );
    // NEL (U+0085) is Rust whitespace but NOT JS's, so it must NOT split →
    // `foo\u{85}bar` is one token, `.foo` does not match it (only the plain
    // `<div class="foo">` matches), so the NEL element is left unscoped.
    let js = compile_js(
        "<div class=\"foo\">a</div>\n<div class=\"foo\u{85}bar\">b</div>\n<style>.foo { color: red }</style>",
    );
    assert!(
        js.contains("class=\"foo svelte-tsvhash\">a</div>"),
        "plain foo scopes: {js:?}"
    );
    assert!(
        js.contains("class=\"foo\u{85}bar\">b</div>"),
        "NEL token must NOT match .foo (no hash): {js:?}"
    );
}

#[test]
fn compile_css_non_ascii_case_insensitive_refuses() {
    // A case-insensitive attribute match with a non-ASCII operand refuses (the
    // oracle folds case with full Unicode; tsv folds ASCII-only — a safe
    // over-refusal). Selector value, element value, and the `i` flag all reach it.
    assert_unsupported(
        "<p data-x=\"caf\u{e9}\">y</p>\n<style>[data-x=\"caf\u{e9}\" i] { color: red }</style>",
        "non-ASCII operand",
    );
    // An HTML case-insensitive attribute (`type`) with a non-ASCII value refuses
    // even without an explicit flag.
    assert_unsupported(
        "<p type=\"caf\u{e9}\">y</p>\n<style>[type=\"caf\u{e9}\"] { color: red }</style>",
        "non-ASCII operand",
    );
    // A case-SENSITIVE compare (no flag, not an HTML ci attr) is a byte test and
    // stays supported with a non-ASCII value.
    let out = compile(
        "<p data-x=\"caf\u{e9}\">y</p>\n<style>[data-x=\"caf\u{e9}\"] { color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("case-sensitive non-ASCII attribute value compiles");
    assert!(out.js.contains("class=\"svelte-tsvhash\""), "{}", out.js);
}

#[test]
fn compile_css_spread_element_scoped_by_type() {
    // A spread element scoped by a type selector carries the hash in the
    // `css_hash` (2nd) `$.attributes` argument (assume-match on the spread too).
    let js = compile_js(
        "<script>let props = $state({});</script>\n<div {...props}>x</div>\n<style>div{ color: red }</style>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, 'svelte-tsvhash')"),
        "{js}"
    );
}
