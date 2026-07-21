//! The `$derived` family: init rewrite, read positions, writes.

use super::support::*;

#[test]
fn compile_derived_rune_rewrites_init_and_read() {
    // `$derived(e)` → `$.derived(() => e)`; a bare template read of the
    // (non-foldable) derived binding becomes `d()`.
    let out = compile_checked(
        "<script>\n\tlet a = $state(1);\n\tlet d = $derived(a * 2);\n\tfunction inc() {\n\t\ta += 1;\n\t}\n</script>\n<p>{d}</p>",
    );
    assert!(
        out.js.contains("let d = $.derived(() => a * 2);"),
        "derived init not rewritten: {}",
        out.js
    );
    assert!(
        out.js.contains("`<p>${$.escape(d())}</p>`"),
        "derived read must become a call: {}",
        out.js
    );
}

#[test]
fn compile_derived_read_refuses_deferred_positions() {
    // The template VALUE walk and the script-position rewrite (`store_rewrite`)
    // turn a derived read into `d()` (the fixtures `runes/derived_read_*` and
    // `runes/derived_read_script_*`). Positions NOT routed through either keep
    // refusing the derived read (`DerivedBindingRead`, "read of derived binding")
    // — never a MISMATCH.
    //
    // A `{#each}` context pattern default: the oracle emits a BARE `d` here
    // (`let { v = d }`), so tsv could match by borrowing verbatim — but patterns are
    // not rewritten this slice, so refusing is a deferred safe over-refusal.
    assert_unsupported(
        "<script>\n\tlet { a, xs } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{#each xs as { v = d }}{v}{/each}",
        "read of derived binding",
    );
    // A `{:then}` value pattern default: here the oracle emits `d()`
    // (`({ x = d() }) => …`), so borrowing the pattern verbatim WOULD emit a bare `d`
    // — a MISMATCH. Refusing is mandatory until patterns route through the walk.
    assert_unsupported(
        "<script>\n\tlet { a, p } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{#await p then { x = d }}{x}{/await}",
        "read of derived binding",
    );
    // A derived assignment target (`{d = 1}`) — the guard refuses the derived
    // WRITE (a template mutation would refuse too). A derived write is out of scope
    // on every path (the oracle lowers it to `d(v)`).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{d = 1}",
        "read of derived binding",
    );
    // A derived read under an ObjectExpression (`{f({ x: d })}`) — a wrapper kind the
    // value walk does not descend, so it never reaches the rewrite and the guard
    // refuses it (a safe over-refusal).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tfunction f(o) {\n\t\treturn o;\n\t}\n</script>\n{f({ x: d })}",
        "read of derived binding",
    );
    // An ESCAPED-identifier read of a `$derived` name: the six source bytes
    // `d` are the escaped spelling of the identifier `d`, which the oracle emits
    // as `d()`. The value-walk can't rewrite an escaped read (classification not
    // ported), so the rune guard refuses it rather than emit a bare `d` — a MISMATCH.
    // Both bare and nested.
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{\\u0064}",
        "read of derived binding",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{\\u0064 + 1}",
        "read of derived binding",
    );
}

#[test]
fn compile_derived_read_script_position_rewrites() {
    // A `$derived` read in a SCRIPT position (a top-level initializer, a function
    // body, a `$.derived(() => …)` thunk) rewrites to `d()`, the same lowering the
    // template value walk applies — extended to the script by `store_rewrite`.
    // Script positions never fold (only template text folds), so it is always
    // `d()`, never the derived's value.
    let out = compile_checked(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet e = d + 1;\n\
         \tfunction total() {\n\t\treturn d + 1;\n\t}\n\tlet d2 = $derived(d + 1);\n\
         </script>\n<button onclick={total}>{a}</button>",
    );
    // The top-level initializer (no fold), the function-body read, and the read
    // inside the `$.derived` thunk all become `d()`; the derived declarations keep
    // their bare binding names.
    assert!(
        out.js.contains("let e = d() + 1;"),
        "top-level init: {}",
        out.js
    );
    assert!(out.js.contains("return d() + 1;"), "fn body: {}", out.js);
    assert!(
        out.js.contains("let d2 = $.derived(() => d() + 1);"),
        "nested-in-derived: {}",
        out.js
    );
    assert!(
        out.js.contains("let d = $.derived(() => a * 2);"),
        "binding id bare: {}",
        out.js
    );
}

#[test]
fn compile_derived_read_name_only_positions_stay_bare() {
    // Name-only positions are NOT reads: a non-computed member property (`o.d`) and
    // an object key (`{ d: 1 }`) stay verbatim, exactly like the store rewrite.
    let out = compile_checked(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction g() {\n\t\tconst o = { d: 1 };\n\t\treturn o.d + d;\n\t}\n\
         </script>\n<button onclick={g}>{a}</button>",
    );
    assert!(
        out.js.contains("const o = { d: 1 };"),
        "object key stays: {}",
        out.js
    );
    assert!(
        out.js.contains("return o.d + d();"),
        "member stays, read rewrites: {}",
        out.js
    );
}

#[test]
fn compile_derived_read_shadowed_refuses() {
    // A `$derived` name shadowed by a nested-scope binding (a param/local) is
    // ambiguous for the name-based rewrite (`return d` inside `f(d)` is the param,
    // not the derived). Refuse the whole compile — a safe over-refusal (shadowing a
    // derived is legal, so this is never a MISMATCH).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction f(d) {\n\t\treturn d;\n\t}\n</script>\n<button onclick={() => f(1)}>{a}</button>",
        "shadowed in a nested scope",
    );
    // A nested local (not a parameter) shadows too.
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction f() {\n\t\tlet d = 5;\n\t\treturn d;\n\t}\n</script>\n<button onclick={f}>{a}</button>",
        "shadowed in a nested scope",
    );
}

#[test]
fn compile_derived_write_refuses() {
    // A write to the derived binding ITSELF (`d = v` / `d++`) is out of scope — the
    // oracle lowers it to `d(v)` / `$.update_derived(d)`, which this slice does not
    // emit. The rune guard refuses the bare-identifier target (`DerivedBindingRead`).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction b() {\n\t\td = 5;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction b() {\n\t\td++;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
    // A destructuring assignment whose leaf binds the derived (`[d] = …`,
    // `({ d } = …)`, `[z, d] = …`) is a derived write too — the oracle lowers it to
    // an `$.to_array` IIFE / `d(obj.d)`. The guard refuses the binding leaf.
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet arr = [1];\n\
         \tfunction b() {\n\t\t[d] = arr;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet obj = { d: 1 };\n\
         \tfunction b() {\n\t\t({ d } = obj);\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet z;\n\tlet arr = [1, 2];\n\
         \tfunction b() {\n\t\t[z, d] = arr;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
}

#[test]
fn compile_derived_member_write_compiles() {
    // A member/index target READS the derived (its object / computed index), never
    // binds it — `d.x = v` → `d().x = v` and `x[d] = v` → `x[d()] = v` compile via
    // the read rewrite (the narrower binding-leaf refusal stops at members).
    let out = compile_checked(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived({ x: a });\n\
         \tfunction b() {\n\t\td.x = 5;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
    );
    assert!(
        out.js.contains("d().x = 5;"),
        "member write reads derived: {}",
        out.js
    );

    let out = compile_checked(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet arr = [1];\n\
         \tfunction b() {\n\t\tarr[d] = 5;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
    );
    assert!(
        out.js.contains("arr[d()] = 5;"),
        "index write reads derived: {}",
        out.js
    );
}

#[test]
fn compile_derived_by_bare_read_compiles() {
    // `$derived.by(d)` (a bare derived argument) compiles: `.by` passes `d` straight
    // through as the compute function (`$.derived(d)`) and the read rewrite lowers
    // it to `$.derived(d())`, the oracle's output. (Contrast `$derived(d)`, which the
    // oracle unthunk-collapses to `$.derived(d)` — refused as unreproducible.)
    let out = compile_checked(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet e = $derived.by(d);\n\
         </script>\n<button onclick={() => e}>{a}</button>",
    );
    assert!(
        out.js.contains("let e = $.derived(d());"),
        "$derived.by(d): {}",
        out.js
    );
}

#[test]
fn compile_escaped_local_read_still_compiles() {
    // An escaped identifier is NOT auto-refused — only one decoding to a `$derived`
    // name is. An escaped read of a plain (non-derived) local compiles, reading the
    // binding bare (`d`, never `d()`).
    let out = compile_checked(
        "<script>\n\tlet { a } = $props();\n\tlet d = a * 2;\n</script>\n{\\u0064}",
    );
    assert!(
        out.js.contains("$.escape(d)"),
        "escaped plain-local read must compile bare: {}",
        out.js
    );
}

#[test]
fn compile_derived_read_state_stays_bare() {
    // Only names in `derived_names` rewrite. A reassigned `$state` binding is NOT
    // derived, so a nested read of it stays bare (`s + 1`, never `s() + 1`).
    let out = compile_checked(
        "<script>\n\tlet s = $state(1);\n\tfunction inc() {\n\t\ts += 1;\n\t}\n</script>\n{s + 1}",
    );
    assert!(
        out.js.contains("$.escape(s + 1)"),
        "state read must stay bare: {}",
        out.js
    );
}

// ── Destructured `$derived` / `$derived.by` (the 1→N lowering) ──────────────

#[test]
fn compile_destructured_derived_object_joins_and_reads_call() {
    // `{a, b} = $derived(o)` → ONE joined declaration, one `$.derived(() => o.KEY)`
    // per leaf; template AND script reads become calls (`a()`).
    let out = compile_checked(
        "<script>\n\tlet o = $props();\n\tlet { a, b } = $derived(o);\n\tconst s = a + b;\n</script>\n{a}{b}",
    );
    assert!(
        out.js
            .contains("let a = $.derived(() => o.a),\n\t\tb = $.derived(() => o.b);"),
        "destructured derived must be one joined declaration: {}",
        out.js
    );
    assert!(
        out.js.contains("const s = a() + b();"),
        "script-position leaf reads must become calls: {}",
        out.js
    );
    assert!(
        out.js.contains("$.escape(a())") && out.js.contains("$.escape(b())"),
        "template leaf reads must become calls: {}",
        out.js
    );
}

#[test]
fn compile_destructured_derived_renamed_and_nested_keys() {
    // A renamed key binds the VALUE (`x`) projecting the KEY (`o.a`); a nested
    // pattern chains the member (`o.a.c`).
    let renamed = compile_js(
        "<script>\n\tlet o = $props();\n\tlet { a: x, b } = $derived(o);\n</script>\n{x}{b}",
    );
    assert!(
        renamed.contains("let x = $.derived(() => o.a),"),
        "renamed key must bind the value projecting the key: {renamed}"
    );
    let nested = compile_js(
        "<script>\n\tlet o = $props();\n\tlet { a: { c } } = $derived(o);\n</script>\n{c}",
    );
    assert!(
        nested.contains("let c = $.derived(() => o.a.c);"),
        "nested pattern must chain the projection: {nested}"
    );
}

#[test]
fn compile_destructured_derived_object_rest_and_default() {
    // A rest projects `$.exclude_from_object(o, [<sibling keys>])`; a simple default
    // wraps `$.fallback`; a non-simple default thunks + collapses (`f()` → `f`).
    let rest = compile_js(
        "<script>\n\tlet o = $props();\n\tlet { a, ...r } = $derived(o);\n</script>\n{a}{r}",
    );
    assert!(
        rest.contains("r = $.derived(() => $.exclude_from_object(o, ['a']));"),
        "rest must exclude the sibling keys: {rest}"
    );
    let simple =
        compile_js("<script>\n\tlet o = $props();\n\tlet { a = 9 } = $derived(o);\n</script>\n{a}");
    assert!(
        simple.contains("let a = $.derived(() => $.fallback(o.a, 9));"),
        "a simple default is a 2-arg fallback: {simple}"
    );
    let complex = compile_js(
        "<script>\n\tlet o = $props();\n\tfunction f() {\n\t\treturn 1;\n\t}\n\tlet { a = f() } = $derived(o);\n</script>\n{a}",
    );
    assert!(
        complex.contains("$.fallback(o.a, f, true)"),
        "a non-simple default thunks + unthunk-collapses: {complex}"
    );
}

#[test]
fn compile_destructured_derived_array_and_collision() {
    // An array mints a `$$derived_array` derived intermediate (read as a call),
    // projecting `()[i]`; a second array collides to `$$derived_array_1`.
    let out = compile_js(
        "<script>\n\tlet o = $props();\n\tlet [a, b] = $derived(o);\n\tlet [c, d] = $derived(o);\n</script>\n{a}{b}{c}{d}",
    );
    assert!(
        out.contains("let $$derived_array = $.derived(() => $.to_array(o, 2)),")
            && out.contains("a = $.derived(() => $$derived_array()[0]),"),
        "array must project through a $$derived_array intermediate: {out}"
    );
    assert!(
        out.contains("let $$derived_array_1 = $.derived(() => $.to_array(o, 2)),"),
        "a second array must collide to $$derived_array_1: {out}"
    );
}

#[test]
fn compile_destructured_derived_array_rest_omits_length() {
    // A trailing rest omits the `$.to_array` length and slices from the index.
    let out = compile_js(
        "<script>\n\tlet o = $props();\n\tlet [a, ...rest] = $derived(o);\n</script>\n{a}{rest}",
    );
    assert!(
        out.contains("$.to_array(o)")
            && out.contains("rest = $.derived(() => $$derived_array().slice(1));"),
        "array rest must omit length and slice: {out}"
    );
}

#[test]
fn compile_destructured_derived_by_and_non_identifier_arg_mint_intermediate() {
    // `$derived.by` (and any non-identifier `$derived` arg) mints `$$d`, projecting
    // from `$$d()`; the `.by` compute fn rides `$.derived(() => …)`.
    let by = compile_js(
        "<script>\n\tlet o = $props();\n\tlet { a } = $derived.by(() => o);\n</script>\n{a}",
    );
    assert!(
        by.contains("let $$d = $.derived(() => o),\n\t\ta = $.derived(() => $$d().a);"),
        "$derived.by destructure must mint $$d: {by}"
    );
    // A member argument mints `$$d` and forces the needs_context wrapper.
    let member =
        compile_js("<script>\n\tlet o = $props();\n\tlet { a } = $derived(o.x);\n</script>\n{a}");
    assert!(
        member.contains("let $$d = $.derived(() => o.x),")
            && member.contains("$$renderer.component("),
        "a member arg mints $$d and wraps: {member}"
    );
    // A call argument unthunk-collapses the intermediate init (`$.derived(getObj)`).
    let call = compile_js(
        "<script>\n\tlet getObj = $props();\n\tlet { a } = $derived(getObj());\n</script>\n{a}",
    );
    assert!(
        call.contains("let $$d = $.derived(getObj),"),
        "a call arg collapses the $$d init via unthunk: {call}"
    );
}

#[test]
fn compile_destructured_derived_from_derived_base_calls_it() {
    // `{x} = $derived(base)` where `base` is itself a derived: no `$$d` (bare
    // identifier), and the store rewrite lowers the projected `base` read to
    // `base()` — `$.derived(() => base().x)`.
    let out = compile_js(
        "<script>\n\tlet n = $props();\n\tlet obj = $derived({ m: 1 });\n\tlet { m } = $derived(obj);\n</script>\n{m}",
    );
    assert!(
        out.contains("let m = $.derived(() => obj().m);"),
        "a derived base must be read as a call inside the projection: {out}"
    );
}

#[test]
fn compile_destructured_derived_refuses_comments() {
    // A carried script comment alongside a destructured derived refuses (the 1→N
    // split is not comment-safe) — a safe over-refusal.
    assert_unsupported(
        "<script>\n\t// note\n\tlet o = { a: 1, b: 2 };\n\tlet { a, b } = $derived(o);\n</script>\n{a}{b}",
        "comments in a script with a destructured $derived declarator",
    );
}

#[test]
fn compile_destructured_derived_refuses_in_multi_declarator() {
    // A destructured derived alongside another declarator in one `let` needs
    // per-source-declarator grouping tsv doesn't reproduce — refuse (the oracle
    // compiles it; a safe over-refusal).
    assert_unsupported(
        "<script>\n\tlet o = $props();\n\tlet x = 1,\n\t\t{ a, b } = $derived(o);\n</script>\n{x}{a}{b}",
        "destructuring a $derived declarator",
    );
}

#[test]
fn compile_destructured_derived_leaf_folds_through_scalar_arg() {
    // A destructured-derived LEAF folds through the rune's argument, exactly like an
    // identifier target — the oracle declares every leaf with the whole `$derived(…)`
    // call as its initial (`scope.js:1204-1213`) and evaluates it through the arg. So
    // with `d`→5 (a bounded scalar), `{a}` folds to the CONTAINER value `5` (ignoring
    // the `.a` projection), NOT a dynamic `$.escape(a())`. This was the committed
    // MISMATCH before the leaf-initial fix (leaves were wrongly `Initial::None`).
    let out = compile_checked(
        "<script>\n\tlet d = $derived(5);\n\tlet { a } = $derived(d);\n</script>\n<p>{a}</p>",
    );
    assert!(
        out.js.contains("<p>5</p>"),
        "destructured-derived scalar-arg leaf must fold to the container value: {}",
        out.js
    );
    assert!(
        !out.js.contains("$.escape"),
        "the folded leaf read must not stay dynamic: {}",
        out.js
    );
    // The transform still lowers each leaf to its own `$.derived(() => path)`; only
    // the template READ folds.
    assert!(
        out.js.contains("let a = $.derived(() => d().a);"),
        "leaf declarator must still be a projecting derived: {}",
        out.js
    );
}

#[test]
fn compile_destructured_derived_object_arg_leaf_stays_dynamic() {
    // The corpus-common case is UNCHANGED by the leaf-initial fix: an object/array
    // argument evaluates to UNKNOWN, so the leaf does NOT fold and reads as `a()`.
    let out = compile_checked(
        "<script>\n\tlet o = { a: 1, b: 2 };\n\tlet { a, b } = $derived(o);\n</script>\n<p>{a}{b}</p>",
    );
    assert!(
        out.js.contains("${$.escape(a())}${$.escape(b())}"),
        "object-arg destructured-derived leaves must stay dynamic calls: {}",
        out.js
    );
}
