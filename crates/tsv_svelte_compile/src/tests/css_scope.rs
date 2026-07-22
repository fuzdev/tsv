//! CSS scoping: selector matching against the element census.

use super::support::*;

#[test]
fn compile_css_type_selector_synthesizes_class() {
    // A bare `<div>` scoped by a type selector gains a synthetic
    // `class="svelte-tsvhash"` (no class markup of its own), and the type selector
    // splices the hash after the tag name.
    let out = compile_checked("<div>x</div>\n<style>div{ color: red }</style>");
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
    let out = compile_checked("<div>x</div>\n<style>*{ color: red }</style>");
    assert_eq!(out.css.as_deref(), Some(".svelte-tsvhash{ color: red }"));
    // `*.c` appends on `.c` (only a bare trailing `*` replaces).
    let out = compile_checked("<div class=\"c\">x</div>\n<style>*.c{ color: red }</style>");
    assert_eq!(out.css.as_deref(), Some("*.c.svelte-tsvhash{ color: red }"));
}

#[test]
fn compile_css_compound_needs_same_element() {
    // `.a.b` matches an element carrying BOTH classes.
    let out = compile_checked("<div class=\"a b\">x</div>\n<style>.a.b{ color: red }</style>");
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
    let out = compile_checked("<div><p>hi</p></div>\n<style>div p{ color: red }</style>");
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
    let out =
        compile_checked("<div class=\"y\">hi</div>\n<style>:global(.x) .y{ color: red }</style>");
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
fn compile_css_dynamic_attribute_value_enumerated_no_match_prunes() {
    // A VALUED attribute selector against a same-named dynamic attribute value the
    // oracle `get_possible_values`-enumerates (an all-literal ternary → {'a','b'})
    // is now ported: neither branch matches `[data-x="z"]`, so the selector matches
    // no element and refuses `CssSelectorNoMatch` (the oracle comment-wraps it) —
    // NOT `CssDynamicAttributeMatch`. Confirms the ternary was enumerated, not
    // assume-matched.
    assert_unsupported(
        "<script>let c = $state(true);</script>\n<p data-x={c ? 'a' : 'b'}>y</p>\n<style>[data-x=\"z\"]{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_dynamic_attribute_logical_and_falsy_bounded() {
    // A non-`class` `&&` injects the falsy quartet (`''`/`false`/`NaN`/`0`), so
    // `[data-x=""]` matches (the `''` member) and the element scopes — but the set
    // is BOUNDED, so a selector for a value not in it (`[data-x="q"]`) still prunes.
    let css = compile_css(
        "<script>let c = $state(true);</script>\n<p data-x={c && 'v'}>y</p>\n<style>[data-x=\"\"]{ color: red }</style>",
    );
    assert_eq!(css, "[data-x=\"\"].svelte-tsvhash{ color: red }");
    assert_unsupported(
        "<script>let c = $state(true);</script>\n<p data-x={c && 'v'}>y</p>\n<style>[data-x=\"q\"]{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_dynamic_attribute_ts_wrapper_erased_before_gather() {
    // The oracle erases TypeScript in phase 1, BEFORE CSS analysis, so `{false as
    // true}` is gathered as `false` → {"false"} — it does NOT match `[data-active=
    // 'true']`, so the selector matches no element and refuses `CssSelectorNoMatch`.
    // Reading the raw `TSAsExpression` (not stripping the wrapper) would fall to
    // `else → UNKNOWN → assume-match` and OVER-scope the element (a MISMATCH the
    // corpus caught). This pins the wrapper strip. Shape = the `unused-ts-as-
    // expression` Svelte suite fixture.
    assert_unsupported(
        "<script lang=\"ts\">\n\t//\n</script>\n<div data-active={false as true}>\n\t<span></span>\n</div>\n<style>[data-active='true'] > span{ background-color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_dynamic_attribute_ts_wrapper_nested_enumerates() {
    // A wrapper nested inside a conditional branch is stripped by the recursion's
    // per-entry strip: `c ? (0 as any) : (1 as any)` enumerates {"0","1"}. `[data-x=
    // "0"]` matches (scopes) while `[data-x="9"]` prunes — confirming the branches
    // were ENUMERATED through the wrappers, not assume-matched.
    let css = compile_css(
        "<script lang=\"ts\">let c = $state(true);</script>\n<p data-x={c ? (0 as any) : (1 as any)}>y</p>\n<style>[data-x=\"0\"]{ color: red }</style>",
    );
    assert_eq!(css, "[data-x=\"0\"].svelte-tsvhash{ color: red }");
    assert_unsupported(
        "<script lang=\"ts\">let c = $state(true);</script>\n<p data-x={c ? (0 as any) : (1 as any)}>y</p>\n<style>[data-x=\"9\"]{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_dynamic_attribute_unstringifiable_literal_refuses() {
    // An otherwise-enumerable set carrying a literal tsv cannot stringify byte-exactly
    // refuses the whole compile (a safe over-refusal — the oracle enumerates it, e.g.
    // `String(0.5)="0.5"`, so tsv declines rather than drop the value and under-match).
    // A non-integer number, a BigInt, and a regex each hit the `CssDynamicAttributeMatch`
    // arm (narrowed to exactly this un-stringifiable residual).
    for value in ["0.5", "10n", "/x/"] {
        assert_unsupported(
            &format!("<p data-x={{{value}}}>y</p>\n<style>[data-x=\"z\"]{{ color: red }}</style>"),
            "un-stringifiable dynamic value",
        );
    }
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
    let out = compile_checked(
        "<p data-x=\"caf\u{e9}\">y</p>\n<style>[data-x=\"caf\u{e9}\"] { color: red }</style>",
    );
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

#[test]
fn compile_css_group_at_rule_scopes_inner_rules() {
    // A non-keyframes group at-rule recurses into its block and scopes the inner
    // rules exactly like top-level ones (the oracle's generic `next()` recursion);
    // the at-rule prelude is untouched. @media with a class inner + a type inner.
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div>\n<style>@media (min-width: 500px) { .a { color: red } div { color: blue } }</style>"
        ),
        "@media (min-width: 500px) { .a.svelte-tsvhash { color: red } div.svelte-tsvhash { color: blue } }"
    );
    // A combinator inside an at-rule bumps specificity per ComplexSelector just like
    // at top level (`.svelte-tsvhash` then `:where(...)`).
    assert_eq!(
        compile_css("<div><p>x</p></div>\n<style>@media screen { div p { color: red } }</style>"),
        "@media screen { div.svelte-tsvhash p:where(.svelte-tsvhash) { color: red } }"
    );
    // Nested at-rules recurse arbitrarily deep.
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div>\n<style>@media screen { @supports (display: grid) { .a { color: red } } }</style>"
        ),
        "@media screen { @supports (display: grid) { .a.svelte-tsvhash { color: red } } }"
    );
}

#[test]
fn compile_css_statement_and_descriptor_at_rules_pass_through_verbatim() {
    // A statement at-rule (`@import`, `block: None`) and a descriptor block
    // (`@font-face`, declarations only) scope nothing and are copied through
    // verbatim by the splicer (it applies edits only from matched inner rules); a
    // sibling `div` rule still scopes.
    assert_eq!(
        compile_css("<div>x</div>\n<style>@import 'x.css'; div { color: red }</style>"),
        "@import 'x.css'; div.svelte-tsvhash { color: red }"
    );
    assert_eq!(
        compile_css(
            "<div>x</div>\n<style>@font-face { font-family: Foo } div { color: red }</style>"
        ),
        "@font-face { font-family: Foo } div.svelte-tsvhash { color: red }"
    );
}

#[test]
fn compile_css_at_rule_prelude_is_never_scoped() {
    // `@scope (.a) to (.b)` — the prelude selectors are a raw string the oracle
    // never scopes; only the inner `.a` rule gains the hash.
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div>\n<style>@scope (.a) to (.b) { .a { color: red } }</style>"
        ),
        "@scope (.a) to (.b) { .a.svelte-tsvhash { color: red } }"
    );
}

#[test]
fn compile_css_at_rule_unused_inner_selector_refuses() {
    // An inner selector matching no element is the oracle's comment-wrap (unused);
    // tsv's existing posture — refuse `CssSelectorNoMatch` — carries through the
    // at-rule descent unchanged (a safe over-refusal).
    assert_unsupported(
        "<div>x</div>\n<style>@media screen { .z { color: red } }</style>",
        "matches no element",
    );
}

// ── @keyframes scoping (the oracle's is_keyframes_node name-prefix + animation-value
// rewrite; every expected string below is the live oracle's CSS, canonical_compile) ──

#[test]
fn compile_css_keyframes_basic_name_prefix_and_animation_ref() {
    // The keyframes NAME is prefixed and every `animation` value token referencing a
    // collected name is rewritten to the prefixed spelling (probe 1).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo { from { opacity: 0 } to { opacity: 1 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo { from { opacity: 0 } to { opacity: 1 } } .a.svelte-tsvhash { animation: svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_global_prefix_is_stripped_and_uncollected() {
    // A `-global-` prelude STRIPS the 8-byte prefix (leaving the bare name, un-scoped)
    // and is NOT collected — so `animation: foo` is left alone (probe 2).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes -global-foo { from { opacity: 0 } to { opacity: 1 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframes foo { from { opacity: 0 } to { opacity: 1 } } .a.svelte-tsvhash { animation: foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_vendor_at_rule_collects() {
    // A vendor-prefixed keyframes at-rule (`@-webkit-keyframes`) DOES collect; the name
    // start is `@` + decoded name (`-webkit-keyframes`, 17 bytes) + 1 (probe 3).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@-webkit-keyframes spin { from { opacity: 0 } to { opacity: 1 } } .a { animation: spin 1s }</style>"
        ),
        "@-webkit-keyframes svelte-tsvhash-spin { from { opacity: 0 } to { opacity: 1 } } .a.svelte-tsvhash { animation: svelte-tsvhash-spin 1s }"
    );
}

#[test]
fn compile_css_keyframes_multi_name_list_in_both_declarations() {
    // Two names, rewritten in BOTH an `animation` list (commas + other tokens) and an
    // `animation-name` list (probe 4).
    assert_eq!(
        compile_css(
            "<div class=\"x\">x</div><style>@keyframes a { from { opacity: 0 } } @keyframes b { from { opacity: 0 } } .x { animation: a 1s ease, b 2s; animation-name: a, b }</style>"
        ),
        "@keyframes svelte-tsvhash-a { from { opacity: 0 } } @keyframes svelte-tsvhash-b { from { opacity: 0 } } .x.svelte-tsvhash { animation: svelte-tsvhash-a 1s ease, svelte-tsvhash-b 2s; animation-name: svelte-tsvhash-a, svelte-tsvhash-b }"
    );
}

#[test]
fn compile_css_keyframes_unused_is_kept_and_prefixed() {
    // Keyframes are never pruned — an unreferenced keyframes is still kept and prefixed
    // (probe 5).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes unused { from { opacity: 0 } to { opacity: 1 } } .a { color: red }</style>"
        ),
        "@keyframes svelte-tsvhash-unused { from { opacity: 0 } to { opacity: 1 } } .a.svelte-tsvhash { color: red }"
    );
}

#[test]
fn compile_css_keyframes_inside_media_collects_and_prefixes() {
    // Keyframes nested in `@media` are prefixed and collected — the `@media` descent
    // reaches them (probe 6).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@media (min-width: 100px) { @keyframes m { from { opacity: 0 } to { opacity: 1 } } } .a { animation: m 1s }</style>"
        ),
        "@media (min-width: 100px) { @keyframes svelte-tsvhash-m { from { opacity: 0 } to { opacity: 1 } } } .a.svelte-tsvhash { animation: svelte-tsvhash-m 1s }"
    );
}

#[test]
fn compile_css_keyframes_declarations_inside_block_are_untouched() {
    // The transform returns without descending into a keyframes block, so an
    // `animation-name` INSIDE it stays verbatim (probe 7).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo { from { animation-name: foo; opacity: 0 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo { from { animation-name: foo; opacity: 0 } } .a.svelte-tsvhash { animation: svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_property_compare_lowercases_but_source_preserved() {
    // The property compare lowercases (`ANIMATION`), but the source property text is
    // preserved — only the value is rewritten (probe 8).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo { from { opacity: 0 } } .a { ANIMATION: foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo { from { opacity: 0 } } .a.svelte-tsvhash { ANIMATION: svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_tab_after_name_glues_the_insert() {
    // Only LITERAL SPACE bytes are skipped after the name, so a TAB leaves the insert
    // glued right after `@keyframes` — GLUED garbage the oracle also emits. The prelude
    // is still trimmed (`foo` collects, `animation: foo` rewrites) (probe 10).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes\tfoo { from { opacity: 0 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframessvelte-tsvhash-\tfoo { from { opacity: 0 } } .a.svelte-tsvhash { animation: svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_empty_prelude_collects_the_empty_string() {
    // An EMPTY prelude collects the empty string: the name insert lands on `{` after the
    // space-skip (probe 13), and the collected `''` matches at every boundary whose
    // accumulated token is empty — here the first boundary after `:` and the final `}`
    // (probe 18). One test pins both.
    assert_eq!(
        compile_css("<div class=\"a\">x</div><style>@keyframes { from { opacity: 0 } }</style>"),
        "@keyframes svelte-tsvhash-{ from { opacity: 0 } }"
    );
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes { from { opacity: 0 } } .a { animation: x 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-{ from { opacity: 0 } } .a.svelte-tsvhash { animation:svelte-tsvhash- x 1s svelte-tsvhash-}"
    );
}

#[test]
fn compile_css_keyframes_quoted_name_not_rewritten() {
    // Quotes are part of the accumulated token, so `"foo"` never equals the collected
    // `foo` — not rewritten (probe 14).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo { from { opacity: 0 } } .a { animation-name: \"foo\" }</style>"
        ),
        "@keyframes svelte-tsvhash-foo { from { opacity: 0 } } .a.svelte-tsvhash { animation-name: \"foo\" }"
    );
}

#[test]
fn compile_css_keyframes_declaration_scan_reaches_font_face_descriptor() {
    // The `Declaration` visitor reaches a descriptor block (`@font-face`), so an
    // `animation-name` there is rewritten (probe 15).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo { from { opacity: 0 } } @font-face { font-family: x; animation-name: foo }</style>"
        ),
        "@keyframes svelte-tsvhash-foo { from { opacity: 0 } } @font-face { font-family: x; animation-name: svelte-tsvhash-foo }"
    );
}

#[test]
fn compile_css_keyframes_reference_before_declaration_is_a_full_prepass() {
    // Collection is a full pre-pass, so an `animation` referencing a keyframes declared
    // LATER in source still rewrites (probe 16).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>.a { animation: late 1s } @keyframes late { from { opacity: 0 } }</style>"
        ),
        ".a.svelte-tsvhash { animation: svelte-tsvhash-late 1s } @keyframes svelte-tsvhash-late { from { opacity: 0 } }"
    );
}

#[test]
fn compile_css_keyframes_no_space_after_colon() {
    // No space between `:` and the name — the token scan still finds it (probe 17).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo { from { opacity: 0 } } .a { animation:foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo { from { opacity: 0 } } .a.svelte-tsvhash { animation:svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_two_word_prelude_collects_the_whole_string() {
    // A two-word prelude collects `'foo bar'` (the whole trimmed prelude), so
    // `animation: foo` does NOT match — but the name TOKEN is still prefixed (probe 19).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo bar { from { opacity: 0 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo bar { from { opacity: 0 } } .a.svelte-tsvhash { animation: foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_only_animation_and_animation_name_rewrite() {
    // `animation-duration` is neither `animation` nor `animation-name`, so its value is
    // left alone (probe 26).
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo { from { opacity: 0 } } .a { animation-duration: foo }</style>"
        ),
        "@keyframes svelte-tsvhash-foo { from { opacity: 0 } } .a.svelte-tsvhash { animation-duration: foo }"
    );
}

#[test]
fn compile_css_keyframes_vendor_prefixed_property_rewrites() {
    // A vendor-prefixed PROPERTY (`-webkit-animation`) is stripped before the compare,
    // so its value rewrites (probe 11); the name-start arithmetic uses the raw property
    // length (17) so the scan begins at the right byte.
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo { from { opacity: 0 } } .a { -webkit-animation: foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo { from { opacity: 0 } } .a.svelte-tsvhash { -webkit-animation: svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_uppercase_keyframes_is_a_group_at_rule() {
    // The keyframes discriminator is case-SENSITIVE (the oracle's `is_keyframes_node`
    // tests `=== 'keyframes'`), so `@KEYFRAMES` is NOT keyframes: it recurses as a
    // group at-rule and its `from`/`to` are element selectors matching no element →
    // `CssSelectorNoMatch` (the oracle likewise comment-wraps them as unused). Either
    // verdict is safe; this pins the one tsv reaches.
    assert_unsupported(
        "<div>x</div>\n<style>@KEYFRAMES spin { from { opacity: 0 } to { opacity: 1 } }</style>",
        "matches no element",
    );
}

// ── @keyframes prelude = the oracle's `node.prelude` (comment-elided, JS-whitespace trim),
// via `CssAtrule::public_prelude` — NOT the printer-facing `Raw::content` (comment-preserving,
// CSS-whitespace trim). When the two diverge, the wrong name silently fails to rewrite the
// `animation` reference while the at-rule name still gets prefixed (renamed keyframes,
// un-renamed reference). Every expected string below is the live oracle's CSS
// (canonical_compile on the exact single-line input). ──

#[test]
fn compile_css_keyframes_prelude_comment_is_elided_for_collection() {
    // Natural real-world CSS: a comment after the name. The oracle's `read_value` elides the
    // comment for `node.prelude` (→ `spin`), so `animation: spin` IS rewritten; the comment
    // survives verbatim in the emitted at-rule (only the name is spliced). `Raw::content`
    // preserved the comment (`spin /* clockwise */`), so the token compare missed `spin`.
    assert_eq!(
        compile_css(
            "<div class=\"spinner\">x</div><style>@keyframes spin /* clockwise */ { from { opacity: 0 } to { opacity: 1 } } .spinner { animation: spin 1s linear infinite }</style>"
        ),
        "@keyframes svelte-tsvhash-spin /* clockwise */ { from { opacity: 0 } to { opacity: 1 } } .spinner.svelte-tsvhash { animation: svelte-tsvhash-spin 1s linear infinite }"
    );
}

#[test]
fn compile_css_keyframes_prelude_vertical_tab_is_trimmed_for_collection() {
    // A vertical tab (U+000B) after the name: JS whitespace (`read_value().trim()` → `foo`)
    // but NOT CSS whitespace, so `Raw::content` kept it (`foo\u{000b}`) and the token compare
    // missed `foo`. The VT survives verbatim after the spliced name.
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo\u{000b} { from { opacity: 0 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo\u{000b} { from { opacity: 0 } } .a.svelte-tsvhash { animation: svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_prelude_nbsp_is_trimmed_for_collection() {
    // A no-break space (U+00A0) after the name: JS whitespace (trimmed → `foo`) but a CSS
    // *ident* code point, so `Raw::content` kept it and the token compare missed `foo`.
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo\u{a0} { from { opacity: 0 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo\u{a0} { from { opacity: 0 } } .a.svelte-tsvhash { animation: svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_prelude_glued_comment_is_elided_for_collection() {
    // A comment glued to the name (`foo/* c */`): the oracle elides it (→ `foo`), the token
    // compare hits, and `animation: foo` is rewritten; the glued comment survives verbatim.
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes foo/* c */ { from { opacity: 0 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframes svelte-tsvhash-foo/* c */ { from { opacity: 0 } } .a.svelte-tsvhash { animation: svelte-tsvhash-foo 1s }"
    );
}

#[test]
fn compile_css_keyframes_animation_in_style_attribute_is_never_rewritten() {
    // The control: `animation` in a `style=` ATTRIBUTE value is never scanned (only `<style>`
    // declarations are), so it stays `foo` even though the `<style>` keyframes is prefixed.
    // Pins that the collection fix did not widen the rewrite into attribute values.
    let out = compile_checked(
        "<div class=\"a\" style=\"animation: foo 1s\">x</div><style>@keyframes foo { from { opacity: 0 } } .a { color: red }</style>",
    );
    assert!(
        out.js.contains("style=\"animation: foo 1s\""),
        "style-attribute animation must stay un-prefixed:\n{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some(
            "@keyframes svelte-tsvhash-foo { from { opacity: 0 } } .a.svelte-tsvhash { color: red }"
        )
    );
}

#[test]
fn compile_css_keyframes_global_hidden_behind_a_comment_still_strips() {
    // `-global-` hides behind a leading comment (`@keyframes /* c */-global-foo`). The oracle
    // elides the comment (`node.prelude` → `-global-foo`), so BOTH the collection exclusion
    // and the STRIP branch fire: the name edit removes 8 bytes at the space-skipped start
    // (`/* c */-`, comment + dash), leaving `global-foo`, and `animation: foo` is left alone.
    // `Raw::content` (`/* c */-global-foo`) missed the `-global-`, so tsv wrongly prefixed.
    assert_eq!(
        compile_css(
            "<div class=\"a\">x</div><style>@keyframes /* c */-global-foo { from { opacity: 0 } } .a { animation: foo 1s }</style>"
        ),
        "@keyframes global-foo { from { opacity: 0 } } .a.svelte-tsvhash { animation: foo 1s }"
    );
}

#[test]
fn compile_refuses_global_pseudo_class_with_a_non_ascii_ident_char() {
    // A CSS name must NOT be trimmed with a Unicode-whitespace notion: every code
    // point at or above U+0080 is a CSS *ident* code point, so `:global\u{a0}` is
    // the pseudo-class `global\u{a0}`, not `:global`. Rust's `str::trim` stripped
    // the NBSP, tsv read `:global`, and scoped the element while the oracle pruned
    // the rule as unused — an oracle-verified MISMATCH, now a safe over-refusal
    // (the selector matches no element).
    assert_unsupported(
        "<div class=\"x\">a</div>\n<style>\n\tdiv :global\u{a0}.x { color: red }\n</style>",
        "matches no element",
    );
}

// ── @keyframes STEP matching (the oracle's prune walk descends into keyframes blocks and
// matches each step rule's selectors against every element — the transform never descends,
// so a step scopes elements without ever splicing the block). Every expected string below
// is the live oracle's output (canonical_compile). ──

#[test]
fn compile_css_keyframes_percentage_step_matches_every_element() {
    // s1: a percentage-only step compound (`0%`/`100%`) has an empty predicate list after the
    // per-simple Percentage skip, so it matches EVERY element (the oracle's fallthrough) —
    // both `<h2>` and `<p>` gain the hash though no selector targets them. The step block is
    // never spliced (only the name is prefixed).
    let out = compile_checked(
        "<h2>x</h2><p>y</p><style>@keyframes k { 0% { opacity: 0 } 100% { opacity: 1 } }</style>",
    );
    assert!(
        out.js
            .contains(r#"<h2 class="svelte-tsvhash">x</h2><p class="svelte-tsvhash">y</p>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { 0% { opacity: 0 } 100% { opacity: 1 } }")
    );
}

#[test]
fn compile_css_keyframes_from_to_steps_scope_only_named_elements() {
    // s2: `from`/`to` are TYPE selectors (they match elements NAMED `from`/`to`), so with a
    // plain `<h2>` present nothing scopes.
    let out = compile_checked(
        "<h2>x</h2><style>@keyframes k { from { opacity: 0 } to { opacity: 1 } }</style>",
    );
    assert!(out.js.contains("<h2>x</h2>"), "{}", out.js);
    assert!(!out.js.contains("svelte-tsvhash\">x"), "{}", out.js);
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { from { opacity: 0 } to { opacity: 1 } }")
    );
}

#[test]
fn compile_css_keyframes_global_percentage_step_still_matches_every_element() {
    // s3: step matching runs for a `-global-` keyframes too (the prune walk is name-blind) —
    // the `0%` step scopes `<h2>`. The NAME edit strips `-global-` (leaving the bare,
    // un-prefixed `k`), independent of step matching.
    let out =
        compile_checked("<h2>x</h2><style>@keyframes -global-k { 0% { opacity: 0 } }</style>");
    assert!(
        out.js.contains(r#"<h2 class="svelte-tsvhash">x</h2>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes k { 0% { opacity: 0 } }")
    );
}

#[test]
fn compile_css_keyframes_media_nested_percentage_step_matches_every_element() {
    // s4: an `@media`-nested keyframes is reached by the group-at-rule descent; its `0%` step
    // scopes `<h2>` and the name is prefixed inside the `@media`.
    let out = compile_checked(
        "<h2>x</h2><style>@media (min-width: 10px) { @keyframes k { 0% { opacity: 0 } } }</style>",
    );
    assert!(
        out.js.contains(r#"<h2 class="svelte-tsvhash">x</h2>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@media (min-width: 10px) { @keyframes svelte-tsvhash-k { 0% { opacity: 0 } } }")
    );
}

#[test]
fn compile_css_keyframes_from_step_scopes_a_from_element() {
    // s5: a `from` step is the TYPE selector `from`, so it scopes a `<from>` element — the
    // step selector matches by tag name, exactly like a top-level type selector.
    let out = compile_checked("<from>x</from><style>@keyframes k { from { opacity: 0 } }</style>");
    assert!(
        out.js.contains(r#"<from class="svelte-tsvhash">x</from>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { from { opacity: 0 } }")
    );
}

#[test]
fn compile_css_keyframes_comma_step_scopes_matching_element_only() {
    // t2: `from, to` is two step ComplexSelectors — `<to>` matches `to`, `<h2>` matches
    // neither, so only `<to>` scopes.
    let out = compile_checked(
        "<to>x</to><h2>y</h2><style>@keyframes k { from, to { opacity: 0 } }</style>",
    );
    assert!(
        out.js
            .contains(r#"<to class="svelte-tsvhash">x</to><h2>y</h2>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { from, to { opacity: 0 } }")
    );
}

#[test]
fn compile_css_keyframes_type_step_scopes_matching_element_only() {
    // t3: a `div` type step scopes `<div>` and leaves `<p>` bare.
    let out =
        compile_checked("<div>x</div><p>y</p><style>@keyframes k { div { opacity: 0 } }</style>");
    assert!(
        out.js
            .contains(r#"<div class="svelte-tsvhash">x</div><p>y</p>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { div { opacity: 0 } }")
    );
}

#[test]
fn compile_css_keyframes_class_step_scopes_matching_element_only() {
    // t4: a `.c` class step scopes the `class="c"` div (appending the hash) and leaves `<p>`
    // bare.
    let out = compile_checked(
        "<div class=\"c\">x</div><p>y</p><style>@keyframes k { .c { opacity: 0 } }</style>",
    );
    assert!(
        out.js
            .contains(r#"<div class="c svelte-tsvhash">x</div><p>y</p>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { .c { opacity: 0 } }")
    );
}

#[test]
fn compile_css_keyframes_percentage_class_compound_narrows_per_simple() {
    // t5: `0%.c` — the Percentage is skipped PER-SIMPLE within the compound, but the `.c`
    // predicate remains, so it matches ONLY `class="c"` (the div), NOT `<p>`. This proves the
    // skip is per-simple, not a blanket "contains a percentage ⇒ scope everything".
    let out = compile_checked(
        "<div class=\"c\">x</div><p>y</p><style>@keyframes k { 0%.c { opacity: 0 } }</style>",
    );
    assert!(
        out.js
            .contains(r#"<div class="c svelte-tsvhash">x</div><p>y</p>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { 0%.c { opacity: 0 } }")
    );
}

#[test]
fn compile_css_keyframes_empty_step_scopes_and_keeps_block_verbatim() {
    // t6: an EMPTY step (`from {}`) does NOT hit the empty-rule refusal (steps are never
    // refused for emptiness) — it still scopes `<from>`, and the CSS keeps `from {}` verbatim
    // (no empty-rule comment-wrap, the transform never descends).
    let out = compile_checked("<from>x</from><style>@keyframes k { from {} }</style>");
    assert!(
        out.js.contains(r#"<from class="svelte-tsvhash">x</from>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { from {} }")
    );
}

#[test]
fn compile_css_keyframes_no_match_step_neither_scopes_nor_refuses() {
    // t7: a `from` step with only an `<h2>` present matches nothing — it NEITHER scopes
    // anything NOR refuses `CssSelectorNoMatch` (steps are never pruned). The component
    // compiles and the keyframes name is still prefixed.
    let out = compile_checked("<h2>x</h2><style>@keyframes k { from { opacity: 0 } }</style>");
    assert!(out.js.contains("<h2>x</h2>"), "{}", out.js);
    assert!(
        !out.js.contains("svelte-tsvhash"),
        "nothing scoped: {}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("@keyframes svelte-tsvhash-k { from { opacity: 0 } }")
    );
}
