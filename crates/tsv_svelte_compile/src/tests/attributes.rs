//! Plain attribute emission: folding, boolean/void attributes, event handlers.

use super::support::*;
use crate::*;

#[test]
fn compile_mixed_attribute_full_fold_emits_static() {
    // Every part of a mixed attribute folding statically emits a STATIC
    // attribute (oracle-probed), not a $.attr*/$.attr_class call: value
    // attr-escaped [&"<] (> stays raw), no trim, boolean attributes keep
    // the folded value, null → ''.
    let js = compile_js(
        "<script>\n\tlet a = 1;\n\tlet b = 2;\n</script>\n\n<div class=\"{a}{b}\"></div>\n",
    );
    assert!(js.contains("`<div class=\"12\"></div>`"), "{js}");
    assert!(!js.contains("$.attr_class"), "must be static: {js}");
    let js = compile_js(
        "<script>\n\tlet a = `x\"y<z>&w`;\n\tlet b = 1;\n</script>\n\n<div title=\"p{a}q{b}\"></div>\n",
    );
    assert!(
        js.contains("`<div title=\"px&quot;y&lt;z>&amp;wq1\"></div>`"),
        "folded value must attr-escape [&\"<] with > raw: {js}"
    );
    let js = compile_js(
        "<script>\n\tlet a = null;\n\tlet b = 1;\n</script>\n\n<input disabled=\"x{a}{b}\" />\n",
    );
    assert!(
        js.contains("disabled=\"x1\""),
        "boolean attr keeps folded value; null folds to '': {js}"
    );
    // A folded-empty class stays `class=""` (the empty-class drop is
    // static-path-only, probe-verified).
    let js = compile_js(
        "<script>\n\tlet a = ``;\n\tlet b = ``;\n</script>\n\n<div class=\"{a}{b}\"></div>\n",
    );
    assert!(js.contains("`<div class=\"\"></div>`"), "{js}");
    // One non-foldable part keeps the whole attribute dynamic with the
    // known parts folded inline (the pre-existing path).
    let js = compile_js(
        "<script>\n\tlet a = 1;\n\tlet { b } = $props();\n</script>\n\n<div title=\"x{a}y{b}\"></div>\n",
    );
    assert!(
        js.contains("$.attr('title', `x1y${$.stringify(b)}`)"),
        "partial fold must stay dynamic: {js}"
    );
}

#[test]
fn compile_empty_class_attribute_drops() {
    // A static string-valued class that collapses+trims to empty is
    // dropped entirely (oracle-probed); a bare `class` (boolean form)
    // keeps `class=""`, and empty style/id stay.
    let js = compile_js("<div class=\"\"></div>\n<div class=\"   \"></div>\n");
    assert!(js.contains("`<div></div> <div></div>`"), "{js}");
    let js = compile_js("<div class></div>\n");
    assert!(js.contains("`<div class=\"\"></div>`"), "{js}");
    let js = compile_js("<div id=\"\" style=\"\" class=\"\" title=\"t\"></div>\n");
    assert!(
        js.contains("`<div id=\"\" style=\"\" title=\"t\"></div>`"),
        "only class drops: {js}"
    );
}

#[test]
fn compile_void_and_boolean_attributes() {
    let out = compile(
        "<p>text1<br />text2</p>\n<input value=\"value\" disabled />",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js
            .contains("`<p>text1<br/>text2</p> <input value=\"value\" disabled=\"\"/>`"),
        "void self-close / boolean attribute wrong: {}",
        out.js
    );
}

#[test]
fn compile_drops_event_handler_attribute() {
    // An `on*` single-expression handler is omitted from SSR output.
    let out = compile(
        "<script>function go() {}</script><button onclick={go}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("`<button>x</button>`") && !out.js.contains("onclick"),
        "event handler not dropped: {}",
        out.js
    );
}

#[test]
fn compile_event_handler_new_forces_wrapper() {
    // A `new` inside a dropped handler still triggers the component wrapper
    // (needs_context walks the handler even though its markup is dropped).
    let out = compile(
        "<button onclick={() => new Date()}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$$renderer.component("),
        "needs_context wrapper missing: {}",
        out.js
    );
}

#[test]
fn compile_event_handler_decision_uses_raw_name() {
    // The oracle's `is_event_attribute` tests the RAW authored name
    // (case-sensitive `startsWith('on')`); lowercasing happens at emission
    // only. So `onClick` drops but `ONCLICK` emits `$.attr('onclick', …)`.
    let out = compile(
        "<script>let { h } = $props();</script><button ONCLICK={h}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.attr('onclick', h)"),
        "ONCLICK must emit, not drop: {}",
        out.js
    );
    let out = compile(
        "<script>let { h } = $props();</script><button onClick={h}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("`<button>x</button>`") && !out.js.contains("onclick"),
        "onClick must drop: {}",
        out.js
    );
    // Raw `onLoad` on a load-error element is a plain drop (the capture
    // exception matches the raw name exactly).
    let out = compile(
        "<script>let { h } = $props();</script><img onLoad={h} src=\"a\" />",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("`<img src=\"a\"/>`"),
        "onLoad on img must plain-drop: {}",
        out.js
    );
    // A mixed-value `ONCLICK` is not an event (raw test) and emits through
    // the normal interpolated-attribute path.
    let out = compile(
        "<script>let { h } = $props();</script><button ONCLICK=\"a {h}\">x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.attr('onclick'"),
        "mixed ONCLICK must emit: {}",
        out.js
    );
}

#[test]
fn compile_handler_shadow_never_masks_the_outer_fold_wrongly() {
    // A handler-local binding (param, destructured/default param, let-decl,
    // function-expr param, nested-arrow param) may own the mutation target,
    // so the outer binding goes Opaque: reads REFUSE (the script side's
    // shadow envelope) rather than fold or escape on a guess.
    for source in [
        "<script>let a = 1;</script><p>{a}</p><button onclick={(a) => a++}>x</button>",
        "<script>let a = 1;</script><p>{a}</p><button onclick={({ a }) => (a = 2)}>x</button>",
        "<script>let a = 1;</script><p>{a}</p><button onclick={(a = 1) => a++}>x</button>",
        "<script>let a = 1;</script><p>{a}</p><button onclick={() => { let a = 0; a++; }}>x</button>",
        "<script>let a = 1;</script><p>{a}</p><button onclick={() => { const f = (a) => a++; f(0); }}>x</button>",
    ] {
        assert_unsupported(source, "binding a is not statically modeled");
    }
    // The non-shadow direction still masks: `(x) => a++` reassigns the
    // OUTER `a`, so its read escapes instead of folding.
    let out = compile(
        "<script>let a = 1;</script><p>{a}</p><button onclick={(x) => a++}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.escape(a)"),
        "outer mutation must escape: {}",
        out.js
    );
    // Partial shadow: the shadowed name refuses only when read; the
    // non-shadowed co-mutated name still masks.
    let out = compile(
            "<script>let a = 1;\n\tlet b = 2;</script><p>{b}</p><button onclick={(a) => { a++; b++; }}>x</button>",
            &CompileOptions::default(),
        )
        .unwrap();
    assert!(
        out.js.contains("$.escape(b)"),
        "co-mutated b must escape: {}",
        out.js
    );
}

#[test]
fn compile_rejects_load_error_event_capture() {
    // `onload`/`onerror` on a load-error element needs `this.__e=event`
    // capture markup, not a clean drop.
    assert_unsupported("<img onload={h} src=\"a\" />", "load-error element");
    assert_unsupported(
        "<iframe onerror={h} src=\"a\"></iframe>",
        "load-error element",
    );
}
