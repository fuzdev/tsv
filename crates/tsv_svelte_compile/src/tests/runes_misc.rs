//! `$effect`, `$inspect`, `$props.id`, `$state.snapshot`, and rune misuse.

use super::support::*;
use crate::*;

#[test]
fn compile_effect_forces_component_wrapper() {
    // Statement-position `$effect(…)` is dropped; the whole body moves
    // inside `$$renderer.component(($$renderer) => { … })`.
    let out = compile(
        "<script>\n\tlet { a } = $props();\n\t$effect(() => {});\n</script>\n<p>{a}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert_eq!(
        out.js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \t$$renderer.component(($$renderer) => {\n\
             \t\tlet { a } = $$props;\n\
             \t\t$$renderer.push(`<p>${$.escape(a)}</p>`);\n\
             \t});\n\
             }\n"
    );
}

#[test]
fn compile_inspect_with_wrong_arity_refuses() {
    // `$inspect(a).with(cb)` drops only with EXACTLY one `.with` argument. A
    // wrong outer arity is a hard oracle error (`rune_invalid_arguments_length`:
    // "`$inspect().with` must be called with exactly one argument"), so the
    // recognizer must not drop it — it falls through to the rune guard.
    assert_unsupported(
        "<script>\n\tlet a = $state(0);\n\t$inspect(a).with();\n</script>\n<p>{a}</p>",
        "$inspect",
    );
    assert_unsupported(
        "<script>\n\tlet a = $state(0);\n\t$inspect(a).with((t, v) => v, 1);\n</script>\n<p>{a}</p>",
        "$inspect",
    );
}

#[test]
fn compile_rejects_rune_in_nested_function() {
    assert_unsupported(
        "<script>\n\tfunction f() {\n\t\tlet c = $state(0);\n\t\treturn c;\n\t}\n</script>\n<p>text</p>",
        "$state",
    );
}

#[test]
fn compile_state_raw_drops_wrapper() {
    // `$state.raw(v)` is a sanctioned init: the wrapper drops; an array
    // value isn't statically foldable, so the read stays dynamic.
    let out = compile(
        "<script>let a = $state.raw([1]);</script>\n<p>{a}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(out.js.contains("let a = [1];"), "got: {}", out.js);
    assert!(
        out.js.contains("`<p>${$.escape(a)}</p>`"),
        "got: {}",
        out.js
    );
}

#[test]
fn compile_rejects_member_form_rune_misuse() {
    // A bare `$props` reference (destructuring the rune itself) refuses.
    assert_unsupported(
        "<script>\n\tlet { id } = $props;\n</script>\n<p>text</p>",
        "$props",
    );
    // A non-sanctioned member-form rune call still refuses (`$props.id()` and
    // `$state.snapshot(x)` are the sanctioned member-form runes; `$state.foo()`
    // is not).
    assert_unsupported(
        "<script>\n\tlet b = $state.foo();\n</script>\n<p>{b}</p>",
        "$state",
    );
}

#[test]
fn compile_props_id_hoists_declaration() {
    // `const id = $props.id()` is skipped in place; a `const id =
    // $.props_id($$renderer)` is hoisted to the top of the component body, and a
    // `{id}` read stays dynamic (`$.escape(id)`, never a fold).
    let out = compile(
        "<script>\n\tconst id = $props.id();\n</script>\n<p>{id}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert_eq!(
        out.js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tconst id = $.props_id($$renderer);\n\
             \t$$renderer.push(`<p>${$.escape(id)}</p>`);\n\
             }\n"
    );
}

#[test]
fn compile_props_id_hoists_before_other_declarators() {
    // In `const a = 1, id = $props.id()` the hoisted `id` decl leads the body,
    // then the surviving `const a = 1` (the oracle's shape).
    let out = compile(
        "<script>\n\tconst a = 1,\n\t\tid = $props.id();\n</script>\n<p>{a}{id}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js
            .contains("const id = $.props_id($$renderer);\n\tconst a = 1;"),
        "props.id decl must lead: {}",
        out.js
    );
}

#[test]
fn compile_props_id_refuses_misuse() {
    // Arguments (`rune_invalid_arguments`) — not recognized as `$props.id()`, so
    // the guard refuses the stray `$props`-rooted call.
    assert_unsupported(
        "<script>\n\tconst id = $props.id(1);\n</script>\n<p>{id}</p>",
        "$props",
    );
    // A destructured target (`props_id_invalid_placement`).
    assert_unsupported(
        "<script>\n\tconst { x } = $props.id();\n</script>\n<p>{x}</p>",
        "$props.id()",
    );
    // A template position (`props_id_invalid_placement`) — the guard refuses it.
    assert_unsupported("<p>{$props.id()}</p>", "$props");
    // A second `$props.id()` (`props_duplicate`).
    assert_unsupported(
        "<script>\n\tconst a = $props.id();\n\tconst b = $props.id();\n</script>\n<p>{a}{b}</p>",
        "more than once",
    );
    // In a module script (`props_id_invalid_placement` — module scope). A plain
    // module now compiles, so the module guard refuses the stray `$props`-rooted
    // call (a module-scope rune) rather than declining the whole module up front.
    assert_unsupported(
        "<script module>\n\tconst id = $props.id();\n</script>\n<p>text</p>",
        "$props",
    );
}

#[test]
fn compile_state_snapshot_declarator_unwraps() {
    // `const s = $state.snapshot(obj)` unwraps to `const s = obj`; the `{s.a}`
    // read stays dynamic.
    let out = compile(
        "<script>\n\tlet obj = $state({ a: 1 });\n\tconst s = $state.snapshot(obj);\n</script>\n<p>{s.a}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(out.js.contains("const s = obj;"), "got: {}", out.js);
    assert!(
        out.js.contains("`<p>${$.escape(s.a)}</p>`"),
        "got: {}",
        out.js
    );
}

#[test]
fn compile_state_snapshot_template_rewrites_to_runtime_call() {
    // A `$state.snapshot(x)` in a template value becomes `$.snapshot(x)`, at the
    // root and nested inside a wrapper expression.
    let bare = compile(
        "<script>\n\tlet obj = $state({ a: 1 });\n</script>\n{$state.snapshot(obj)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        bare.js.contains("$.escape($.snapshot(obj))"),
        "bare snapshot: {}",
        bare.js
    );
    let nested = compile(
        "<script>\n\tlet state = $state({ a: 1 });\n</script>\n{2 in $state.snapshot(state)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        nested.js.contains("$.escape(2 in $.snapshot(state))"),
        "nested snapshot: {}",
        nested.js
    );
}

#[test]
fn compile_state_snapshot_derived_arg_becomes_call() {
    // A bare derived read as the snapshot argument becomes `d()` inside the
    // `$.snapshot(...)` call.
    let out = compile(
        "<script>\n\tlet a = $state(1);\n\tlet d = $derived(a * 2);\n</script>\n{$state.snapshot(d)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.escape($.snapshot(d()))"),
        "got: {}",
        out.js
    );
    // A NESTED derived read inside the snapshot argument (`d + 1`) also rewrites —
    // the snapshot walk and the derived-read walk compose on one node set.
    let nested = compile(
        "<script>\n\tlet a = $state(1);\n\tlet d = $derived(a * 2);\n</script>\n{$state.snapshot(d + 1)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        nested.js.contains("$.escape($.snapshot(d() + 1))"),
        "nested derived in snapshot arg: {}",
        nested.js
    );
}

#[test]
fn compile_state_snapshot_refuses_wrong_arity_and_deferred_positions() {
    // Arity ≠ 1 (`rune_invalid_arguments_length`) — not recognized as snapshot,
    // so the guard refuses the stray `$state`-rooted call.
    assert_unsupported(
        "<script>\n\tlet o = $state({ a: 1 });\n</script>\n{$state.snapshot()}",
        "$state",
    );
    assert_unsupported(
        "<script>\n\tlet o = $state({ a: 1 });\n</script>\n{$state.snapshot(o, 1)}",
        "$state",
    );
    // A destructured declarator (the oracle's temp-destructure lowering) — a safe
    // over-refusal.
    assert_unsupported(
        "<script>\n\tlet obj = $state({ a: 1 });\n\tconst { a } = $state.snapshot(obj);\n</script>\n<p>{a}</p>",
        "$state.snapshot",
    );
    // A script non-declarator position (deferred this slice) — the guard refuses it.
    assert_unsupported(
        "<script>\n\tlet x = $state(1);\n\tfunction f() {\n\t\treturn $state.snapshot(x);\n\t}\n</script>\n<p>text</p>",
        "$state",
    );
}

#[test]
fn compile_rune_optional_chain_declarator_refuses() {
    // An optional-chained rune init (`$state.snapshot?.(x)`, `$state?.snapshot(x)`,
    // `$props.id?.()`, `$state?.(1)`, …) is a ChainExpression the oracle's
    // `get_rune` does not see through, so its declarator-unwrap never applies. tsv
    // refuses to classify the optional form (a safe over-refusal) — closing a
    // net-new MISMATCH (`$state.snapshot?.()`, where the oracle emits
    // `$.snapshot(x)` and unwrapping to `x` diverged) and a pre-existing
    // optional-chain over-acceptance family for the placement-restricted runes
    // (the oracle rejects those, tsv used to compile them). Both the
    // optional-call and optional-member spellings, over every declarator-unwrap
    // rune.
    // Each source is paired with the rune the refusal must name: an
    // `is_err()`-style assertion would pass on a PARSE error too, and would merge
    // these three distinct refusals into one.
    for (src, rune) in [
        (
            "<script>\n\tlet o = $state({ a: 1 });\n\tconst s = $state.snapshot?.(o);\n</script>\n<p>{s.a}</p>",
            "rune $state",
        ),
        (
            "<script>\n\tlet o = $state({ a: 1 });\n\tconst s = $state?.snapshot(o);\n</script>\n<p>{s.a}</p>",
            "rune $state",
        ),
        (
            "<script>\n\tconst id = $props.id?.();\n</script>\n<p>{id}</p>",
            "rune $props",
        ),
        (
            "<script>\n\tconst x = $state?.(1);\n</script>\n<p>{x}</p>",
            "rune $state",
        ),
        (
            "<script>\n\tconst p = $props?.();\n</script>\n<p>text</p>",
            "rune $props",
        ),
        (
            "<script>\n\tconst d = $derived?.(1);\n</script>\n<p>{d}</p>",
            "rune $derived",
        ),
    ] {
        assert_unsupported(src, rune);
    }
}

#[test]
fn compile_state_snapshot_optional_chain_template_still_parity() {
    // In a TEMPLATE value position the optional form is fine: the oracle emits
    // `$.snapshot(x)` regardless of the `?.`, and `snapshot_call_arg` matches it,
    // so tsv emits the same — the declarator guard above does NOT reach here.
    let out = compile(
        "<script>\n\tlet o = $state({ a: 1 });\n</script>\n{$state.snapshot?.(o)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.escape($.snapshot(o))"),
        "got: {}",
        out.js
    );
}

#[test]
fn compile_rejects_rune_in_arrow_and_template_expression() {
    assert_unsupported(
        "<script>\n\tconst f = () => $inspect(1);\n</script>\n<p>text</p>",
        "$inspect",
    );
    assert_unsupported("<p>{$state(0)}</p>", "$state");
    // A rune buried inside a foldable expression must refuse — the guard
    // runs before evaluation, so the fold can't paper over it.
    assert_unsupported("<p>{true ? 1 : $state(2)}</p>", "$state");
}

#[test]
fn compile_exponentiation_fold_matches_js_semantics() {
    // ECMAScript `**` special cases (oracle-verified): a NaN exponent and
    // |base| == 1 with an infinite exponent both fold to NaN, where IEEE
    // `pow` would give 1.
    for source in [
        "<p>{1 ** (1 / 0)}</p>",
        "<p>{(0 - 1) ** (1 / 0)}</p>",
        "<p>{1 ** (0 / 0)}</p>",
    ] {
        let out = compile(source, &CompileOptions::default()).unwrap();
        assert!(
            out.js.contains("`<p>NaN</p>`"),
            "{source} must fold to NaN: {}",
            out.js
        );
    }
    // The plain case stays IEEE.
    let out = compile("<p>{2 ** 3}</p>", &CompileOptions::default()).unwrap();
    assert!(out.js.contains("`<p>8</p>`"), "got: {}", out.js);
}
