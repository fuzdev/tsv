//! Store access: reads, writes, and the subscription injection.

use super::support::*;

#[test]
fn compile_store_read_subscribes() {
    // A template `$name` read where `name` is a binding is a store
    // auto-subscription: `$.store_get(($$store_subs ??= {}), '$name', name)`, plus
    // the `var $$store_subs;` header and the `$.unsubscribe_stores` cleanup,
    // injected at the component-body level (no wrapper forced on its own).
    let out =
        compile_checked("<script>\n\timport { count } from './s';\n</script>\n<p>{$count}</p>");
    assert!(
        out.js
            .contains("$.store_get(($$store_subs ??= {}), '$count', count)"),
        "store read: {}",
        out.js
    );
    assert!(out.js.contains("var $$store_subs;"), "subs var: {}", out.js);
    assert!(
        out.js
            .contains("if ($$store_subs) $.unsubscribe_stores($$store_subs);"),
        "unsubscribe: {}",
        out.js
    );
    assert!(
        !out.js.contains("$$renderer.component"),
        "a bare store read must not force the wrapper: {}",
        out.js
    );
}

#[test]
fn compile_store_script_reads_and_writes() {
    // Script-position reads, writes, and updates all compile now: a read →
    // `$.store_get`, an assignment → `$.store_set`, an update → `$.update_store`.
    let read = compile_js(
        "<script>\n\timport { count } from './s';\n\tconst d = $count * 2;\n</script>\n<p>{d}</p>",
    );
    assert!(
        read.contains("const d = $.store_get(($$store_subs ??= {}), '$count', count) * 2"),
        "script read: {read}"
    );
    let write = compile_js(
        "<script>\n\timport { count } from './s';\n\tfunction f() { $count = 5; }\n</script>\n<button onclick={f}>{$count}</button>",
    );
    assert!(
        write.contains("$.store_set(count, 5)"),
        "store write: {write}"
    );
    let update = compile_js(
        "<script>\n\timport { count } from './s';\n\tfunction f() { $count++; }\n</script>\n<button onclick={f}>{$count}</button>",
    );
    assert!(
        update.contains("$.update_store(($$store_subs ??= {}), '$count', count)"),
        "store update: {update}"
    );
    // A store read in CALLEE / new position (`$fn()`, `new $C()`) is rewritten too
    // (it forces the needs_context wrapper — a call rooted at the import).
    let callee = compile_js(
        "<script>\n\timport { fn } from './s';\n\tfunction f() { return $fn(); }\n</script>\n{f()}",
    );
    assert!(
        callee.contains("$.store_get(($$store_subs ??= {}), '$fn', fn)()"),
        "callee store read: {callee}"
    );
    let new_call = compile_js(
        "<script>\n\timport { C } from './s';\n\tfunction f() { return new $C(); }\n</script>\n{f()}",
    );
    assert!(
        new_call.contains("new ($.store_get(($$store_subs ??= {}), '$C', C))()"),
        "new store read: {new_call}"
    );
}

#[test]
fn compile_store_write_refuses_member_and_destructuring() {
    // A member write (`$obj.foo = 5` → `$.store_mutate`) and a destructuring write
    // (`[$count] = arr` → an IIFE) are out of scope for this slice — refuse rather
    // than emit the un-ported lowering.
    assert_unsupported(
        "<script>\n\timport { obj } from './s';\n\tfunction f() { $obj.foo = 5; }\n</script>\n<button onclick={f}>x</button>",
        "store member write",
    );
    assert_unsupported(
        "<script>\n\timport { count } from './s';\n\tfunction f(arr) { [$count] = arr; }\n</script>\n<button onclick={f}>x</button>",
        "store destructuring write",
    );
    // A member UPDATE (`$obj.foo++`) refuses the same way.
    assert_unsupported(
        "<script>\n\timport { obj } from './s';\n\tfunction f() { $obj.foo++; }\n</script>\n<button onclick={f}>x</button>",
        "store member write",
    );
}

#[test]
fn compile_store_refuses_scoped_subscription() {
    // `$count` where the base `count` is bound in a nested scope is the oracle's
    // `store_invalid_scoped_subscription` error. Refuse (name-based shadow check).
    assert_unsupported(
        "<script>\n\timport { writable } from 'svelte/store';\n\tlet count = writable(0);\n\tfunction f(count) { return $count; }\n</script>\n<p>{f}</p>",
        "not a top-level component binding",
    );
    // A base that is not a component binding at all (`$missing`) stays refused as a
    // bare `$`-prefixed identifier (the oracle's `global_reference_invalid`).
    assert_unsupported(
        "<script>\n\tfunction f(count) { return $count; }\n</script>\n<p>{f}</p>",
        "$count",
    );
    // A store read in CALLEE position with a shadowed LOCAL base refuses via the
    // same `StoreScopedSubscription` path as a bare read (the callee-exemption in
    // the guard mirrors the bare-read shadow handling). A local base keeps
    // `needs_context` out of the way — a `$fn()` rooted at an IMPORT instead
    // refuses earlier as `MemberCallAmbiguousRoot` (also a refusal, no
    // over-acceptance).
    assert_unsupported(
        "<script>\n\timport { writable } from 'svelte/store';\n\tlet fn = writable(0);\n\tfunction f(fn) { return $fn(); }\n</script>\n<p>{f}</p>",
        "not a top-level component binding",
    );
    // The coordinator's literal example — a callee whose base is a bare param
    // (not a top-level binding) — refuses as a rune call ($fn is not a store base).
    assert_unsupported(
        "<script>\n\tfunction f(fn) { return $fn(); }\n</script>\n<p>{f}</p>",
        "rune $fn",
    );
    // A genuine rune call in callee position stays refused (never exempted).
    assert_unsupported(
        "<script>\n\tlet x = $state(0);\n\tfunction f() { return $state(1); }\n</script>\n<p>{f}</p>",
        "rune",
    );
}

#[test]
fn compile_dollar_rune_is_not_a_store_read() {
    // A rune callee (`$props()`) is NOT a store read even when its base name
    // coincides with a binding (`const props = $props()`): stripping `$props` to
    // `props` and treating it as a store on the props object would spuriously force
    // the `$$renderer.component` wrapper. Regression guard for `store_read_base`.
    let out = compile_checked("<script>\n\tconst props = $props();\n</script>\n<p>text</p>");
    assert!(
        !out.js.contains("$$store_subs"),
        "$props() must not mint store subscriptions: {}",
        out.js
    );
}

#[test]
fn compile_store_read_in_snippet_stays_nested() {
    // A top-level `{#snippet}` whose only hoist-blocking reference is a store read
    // must NOT hoist to module scope — its body reads the component-local
    // `$$store_subs`. (Regression: the free-var collector recorded `$count`, which
    // is not a binding name, so the store read failed to block hoisting.)
    let out = compile_checked(
        "<script>\n\timport { count } from './s';\n</script>\n{#snippet foo()}{$count}{/snippet}{@render foo()}",
    );
    // The snippet function nests inside the component (after `var $$store_subs;`),
    // never as a module-scope sibling of the import.
    let subs = out.js.find("var $$store_subs;").expect("subs var");
    let foo = out.js.find("function foo").expect("snippet fn");
    assert!(
        foo > subs,
        "snippet must stay nested (after $$store_subs), got:\n{}",
        out.js
    );
}

#[test]
fn compile_derived_store_reads_call() {
    // A store whose base is a `$derived` binding reads `d()` as the store value —
    // a `$derived` read is a call at every position.
    let out = compile_checked("<script>\n\tlet d = $derived(0);\n</script>\n<p>{$d}</p>");
    assert!(
        out.js
            .contains("$.store_get(($$store_subs ??= {}), '$d', d())"),
        "derived-base store must read d(): {}",
        out.js
    );
}

#[test]
fn compile_escaped_store_read_subscribes() {
    // A `$count` store read written with a unicode escape (`$count` decodes to
    // `$count`; `$` = `$`) is the SAME store auto-subscription the oracle sees
    // — it decodes `node.name`, so the escaped spelling emits byte-identically to
    // the plain one: `$.store_get(($$store_subs ??= {}), '$count', count)` plus the
    // subscription scaffold. (Corpus-absent — nobody writes escaped identifiers — so
    // no gate catches a divergence here; this pins the decode.)
    let out = compile_checked(
        "<script>\n\timport { count } from './s';\n</script>\n<p>{\\u0024count}</p>",
    );
    assert!(
        out.js
            .contains("$.store_get(($$store_subs ??= {}), '$count', count)"),
        "escaped store read: {}",
        out.js
    );
    assert!(out.js.contains("var $$store_subs;"), "subs var: {}", out.js);
    assert!(
        out.js
            .contains("if ($$store_subs) $.unsubscribe_stores($$store_subs);"),
        "unsubscribe: {}",
        out.js
    );
    // CONTROL: the escaped spelling must compile byte-identically to the plain one
    // — the decode must not perturb the hot store path.
    let plain = compile_js("<script>\n\timport { count } from './s';\n</script>\n<p>{$count}</p>");
    let escaped =
        compile_js("<script>\n\timport { count } from './s';\n</script>\n<p>{\\u0024count}</p>");
    assert_eq!(
        plain, escaped,
        "escaped `$count` must compile identically to plain `$count`"
    );
}

#[test]
fn compile_escaped_store_script_read_and_writes() {
    // Script-position escaped reads / writes / updates lower exactly as the plain
    // spellings: a read → `$.store_get`, a write → `$.store_set`, an update →
    // `$.update_store`.
    let read = compile_js(
        "<script>\n\timport { count } from './s';\n\tconst x = \\u0024count;\n</script>\n<p>{x}</p>",
    );
    assert!(
        read.contains("$.store_get(($$store_subs ??= {}), '$count', count)"),
        "escaped script read: {read}"
    );
    let write = compile_js(
        "<script>\n\timport { count } from './s';\n\tfunction f() { \\u0024count = 5; }\n</script>\n<button onclick={f}>{$count}</button>",
    );
    assert!(
        write.contains("$.store_set(count, 5)"),
        "escaped store write: {write}"
    );
    let update = compile_js(
        "<script>\n\timport { count } from './s';\n\tfunction f() { \\u0024count++; }\n</script>\n<button onclick={f}>{$count}</button>",
    );
    assert!(
        update.contains("$.update_store(($$store_subs ??= {}), '$count', count)"),
        "escaped store update: {update}"
    );
}

#[test]
fn compile_escaped_store_write_scaffolds_subscription() {
    // An escaped store WRITE in a DROPPED handler emits no `$.store_set` (SSR drops
    // the handler) but still forces the `var $$store_subs;` / `$.unsubscribe_stores`
    // scaffold via the analysis-driven `uses_stores` gate — which now counts the
    // decoded escaped `$count` reference, exactly as the oracle's does.
    let out = compile_checked(
        "<script>\n\timport { count } from './s';\n</script>\n<button onclick={() => \\u0024count = 5}>go</button>",
    );
    assert!(
        out.js.contains("var $$store_subs;"),
        "escaped dropped write must scaffold subscription: {}",
        out.js
    );
    assert!(
        out.js
            .contains("if ($$store_subs) $.unsubscribe_stores($$store_subs);"),
        "unsubscribe: {}",
        out.js
    );
}

#[test]
fn compile_escaped_store_destructuring_write_refuses() {
    // An escaped destructuring store write (`[$count] = arr`) refuses as
    // `StoreDestructuringWrite`, exactly as the plain `[$count] = arr` does — the
    // pattern-target detection decodes the escaped leaf, so it never falls through
    // to corrupt the assignment target (before the decode this silently MISMATCHED).
    assert_unsupported(
        "<script>\n\timport { count } from './s';\n\tfunction f(arr) { [\\u0024count] = arr; }\n</script>\n<button onclick={f}>x</button>",
        "store destructuring write",
    );
}

#[test]
fn compile_escaped_shadowed_store_base_refuses() {
    // An escaped store read whose decoded base is shadowed by a nested scope is the
    // oracle's `store_invalid_scoped_subscription` — the decode makes `store_base`
    // see the shadowed base, so it refuses exactly as the plain form does (rather
    // than fall through and corrupt).
    assert_unsupported(
        "<script>\n\timport { writable } from 'svelte/store';\n\tlet count = writable(0);\n\tfunction f(count) { return \\u0024count; }\n</script>\n<p>{f}</p>",
        "not a top-level component binding",
    );
}

#[test]
fn compile_escaped_slots_subscribes() {
    // `$$slots` written with a unicode escape (`$$slots`) is the oracle's
    // `uses_slots` reference — it decodes `node.name` — so it injects
    // `const $$slots = $.sanitize_slots($$props)` and reads `$$slots`, matching the
    // plain spelling.
    let out = compile_checked("<script>\n\tlet x = 1;\n</script>\n{\\u0024\\u0024slots}");
    assert!(
        out.js.contains("$.sanitize_slots($$props)"),
        "escaped $$slots must inject sanitize_slots: {}",
        out.js
    );
    assert!(
        out.js.contains("$.escape($$slots)"),
        "escaped $$slots read: {}",
        out.js
    );
}

#[test]
fn compile_shadowed_store_base_refuses() {
    // A store base shadowed by a block-local (`{#each}`/`{#await}`/snippet param) is
    // not a top-level store — the oracle errors `store_invalid_scoped_subscription`,
    // so tsv refuses rather than subscribe to the block-local.
    assert_unsupported(
        "<script>\n\timport { count } from './s';\n</script>\n{#each xs as count}{$count}{/each}",
        "$count",
    );
    assert_unsupported(
        "<script>\n\timport { count } from './s';\n</script>\n{#snippet foo(count)}{$count}{/snippet}{@render foo(1)}",
        "$count",
    );
}
