//! Instance-script rewrites: declarator splits, props injection, export refusals.

use super::support::*;

#[test]
fn compile_splits_multi_declarator_declaration() {
    // The oracle splits a multi-declarator top-level declaration into one
    // declaration per declarator, source order preserved.
    let js = compile_js("<script>let a = 1, b = a + 1;</script>\n<p>x</p>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tlet a = 1;\n\
             \tlet b = a + 1;\n\
             \t$$renderer.push(`<p>x</p>`);\n\
             }\n"
    );
}

#[test]
fn compile_splits_mixed_rune_and_plain_declarators() {
    // The per-declarator rune rewrites compose with the split.
    let js = compile_js(
        "<script>let a = $state(1), d = $derived(a * 2);\n\tfunction f() {\n\t\ta++;\n\t}</script>\n<p>{d}</p>",
    );
    assert!(
        js.contains("\tlet a = 1;\n\tlet d = $.derived(() => a * 2);\n"),
        "mixed declarators must split with rewrites applied: {js}"
    );
}

#[test]
fn compile_keeps_nested_multi_declarator_joined() {
    // Only instance-script top-level declarations split; a declaration
    // inside a function body stays joined as ONE statement (the oracle
    // leaves it alone). The canonical reprint breaks its declarators across
    // continuation lines (multi-init declarations always break) — the same
    // on both sides of the parity diff, so still one `let`.
    let js = compile_js(
        "<script>function f() {\n\t\tlet a = 1,\n\t\t\tb = 2;\n\t\treturn a + b;\n\t}</script>\n<p>{f()}</p>",
    );
    assert!(
        js.contains("let a = 1,\n\t\t\tb = 2;"),
        "nested declaration must stay one statement: {js}"
    );
    assert_eq!(
        js.matches("let").count(),
        1,
        "nested declaration must not split: {js}"
    );
}

#[test]
fn compile_refuses_comment_with_multi_declarator() {
    // The oracle re-anchors a comment INSIDE the split (`let // c` then the
    // declarator on the next line) — not reproducible, refuse.
    assert_unsupported(
        "<script>\n\t// lead\n\tlet a = 1, b = 2;\n</script>\n<p>x</p>",
        "multi-declarator declaration",
    );
}

#[test]
fn compile_refuses_instance_script_exports() {
    // Every instance-script export form refuses: the oracle compiles
    // `export const`/`function`/`{a}` via `$.bind_props` (not implemented),
    // rejects `export default`/`export let` (runes mode), and drops
    // `export * from` — a verbatim passthrough would nest an `export`
    // inside the component function (invalid JS).
    for source in [
        "<script>export const a = 1;</script>\n<p>x</p>",
        "<script>export let a = 1;</script>\n<p>x</p>",
        "<script>export var a = 1;</script>\n<p>x</p>",
        "<script>export function f() {}</script>\n<p>x</p>",
        "<script>export class C {}</script>\n<p>x</p>",
        "<script>let a = 1;\n\texport { a };</script>\n<p>x</p>",
        "<script>export default 5;</script>\n<p>x</p>",
        "<script>export * from './x.js';</script>\n<p>x</p>",
        "<script>export { a } from './x.js';</script>\n<p>x</p>",
    ] {
        assert_unsupported(source, "instance-script export");
    }
}

#[test]
fn compile_refuses_top_level_legacy_reactive_statement() {
    // A top-level `$:` label is a legacy reactive statement — the oracle
    // rejects it in runes mode (legacy_reactive_statement_invalid), so
    // cloning it through as a dead JS label would be a silent mis-compile.
    for source in [
        "<script>let c = 1;\n\t$: doubled = c * 2;</script>\n<p>{c}</p>",
        "<script>let c = 1;\n\t$: { console.log(c); }</script>\n<p>{c}</p>",
        "<script>let c = 1;\n\t$: if (c) c = 0;</script>\n<p>{c}</p>",
    ] {
        assert_unsupported(source, "legacy reactive statement");
    }
}

#[test]
fn compile_refuses_runes_invalid_imports() {
    // The oracle's runes-mode import rules: `svelte/internal*` sources are
    // forbidden outright; `beforeUpdate`/`afterUpdate` cannot be imported
    // from `svelte`. Other `svelte` imports stay valid.
    for source in [
        "<script>import { get } from 'svelte/internal/client';</script>\n<p>x</p>",
        "<script>import * as i from 'svelte/internal';</script>\n<p>x</p>",
    ] {
        assert_unsupported(source, "import from svelte/internal");
    }
    for source in [
        "<script>import { beforeUpdate } from 'svelte';</script>\n<p>x</p>",
        "<script>import { afterUpdate as au } from 'svelte';</script>\n<p>x</p>",
    ] {
        assert_unsupported(source, "runes-invalid import");
    }
    let js = compile_js("<script>import { mount } from 'svelte';</script>\n<p>x</p>");
    assert!(
        js.contains("import { mount } from 'svelte';"),
        "valid svelte import must hoist through: {js}"
    );
}

#[test]
fn compile_clones_plain_and_nested_dollar_labels() {
    // A plain label anywhere, and a `$` label INSIDE a function, are
    // ordinary JS the oracle accepts and clones through verbatim — only
    // the top-level `$:` form is the legacy reactive statement.
    let js = compile_js(
        "<script>\n\touter: for (let i = 0; i < 1; i += 1) {\n\t\tbreak outer;\n\t}\n</script>\n<p>x</p>",
    );
    assert!(
        js.contains("outer: for"),
        "plain label must clone through: {js}"
    );
    let js = compile_js(
        "<script>\n\tlet c = 1;\n\tfunction f() {\n\t\t$: y = c;\n\t\treturn y;\n\t}\n</script>\n<p>{c}</p>",
    );
    assert!(
        js.contains("$: y = c;"),
        "nested $ label must clone through: {js}"
    );
}

#[test]
fn compile_injects_slots_events_before_props_rest() {
    // A rest element in the `$props()` pattern gains the oracle's
    // `$$slots, $$events` injection immediately before it.
    let js = compile_js("<script>let { a, ...rest } = $props();</script>\n<p>{a}</p>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { a, $$slots, $$events, ...rest } = $$props;\n\
             \t$$renderer.push(`<p>${$.escape(a)}</p>`);\n\
             }\n"
    );
}

#[test]
fn compile_wraps_non_destructured_props_in_rest_pattern() {
    // `let props = $props()` becomes the oracle's
    // `let { $$slots, $$events, ...props } = $$props;`.
    let js = compile_js("<script>let props = $props();</script>\n<p>x</p>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { $$slots, $$events, ...props } = $$props;\n\
             \t$$renderer.push(`<p>x</p>`);\n\
             }\n"
    );
}

#[test]
fn compile_plain_props_destructure_gets_no_injection() {
    // No rest element → no `$$slots`/`$$events` (probe-verified).
    let js = compile_js("<script>let { a } = $props();</script>\n<p>{a}</p>");
    assert!(
        !js.contains("$$slots") && !js.contains("$$events"),
        "plain destructure must not gain the injection: {js}"
    );
}

#[test]
fn compile_refuses_props_injection_with_comments() {
    // The injected properties' appendix spans between host-span siblings
    // would sweep host comments — refuse.
    assert_unsupported(
        "<script>\n\t// note\n\tlet { a, ...rest } = $props();\n</script>\n<p>{a}</p>",
        "rest-element $props()",
    );
    assert_unsupported(
        "<script>\n\t// note\n\tlet props = $props();\n</script>\n<p>x</p>",
        "non-destructured $props()",
    );
}

#[test]
fn compile_refuses_array_pattern_props() {
    // The oracle rejects a non-identifier/non-object `$props()` binding
    // (props_invalid_identifier) — refuse rather than compile it.
    assert_unsupported(
        "<script>let [a] = $props();</script>\n<p>x</p>",
        "$props() binding pattern",
    );
}

#[test]
fn compile_allows_lang_js_and_empty() {
    // The oracle compiles `lang="js"` and `lang=""` exactly like no lang
    // attribute; other values stay refused.
    for source in [
        "<script lang=\"js\">let x = 5;</script>\n<p>text</p>",
        "<script lang=\"\">let x = 5;</script>\n<p>text</p>",
    ] {
        let _ = compile_js(source);
    }
    assert_unsupported(
        "<script lang=\"coffee\">let x = 5;</script>\n<p>text</p>",
        "lang=\"coffee\"",
    );
}

#[test]
fn compile_rejects_option_and_populated_select() {
    // The oracle compiles <option> into $$renderer.option closures, and a
    // populated <select>/<optgroup> gets a `<!>` anchor — static emission
    // would diverge.
    assert_unsupported("<option value=\"a\">text</option>", "<option>");
    assert_unsupported(
        "<datalist><option value=\"a\">text</option></datalist>",
        "<option>",
    );
    assert_unsupported("<select><p>text</p></select>", "<select> with children");
    assert_unsupported(
        "<optgroup><p>text</p></optgroup>",
        "<optgroup> with children",
    );
}

#[test]
fn compile_allows_empty_select() {
    // An empty <select> emits statically and matches the oracle.
    let out = compile_checked("<select name=\"n\"></select>");
    assert!(
        out.js.contains("`<select name=\"n\"></select>`"),
        "got: {}",
        out.js
    );
}
