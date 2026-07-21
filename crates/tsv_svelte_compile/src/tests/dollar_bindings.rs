//! The `$`-prefixed identifier rules and the oracle exemptions.

use super::support::*;

#[test]
fn compile_rejects_bare_rune_reference() {
    // A bare $-prefixed identifier reference is oracle-rejected input —
    // refuse instead of compiling a broken passthrough.
    assert_unsupported(
        "<script>\n\tlet x = $state;\n</script>\n<p>text</p>",
        "$state",
    );
    assert_unsupported("<p>{$foo}</p>", "$foo");
}

#[test]
fn compile_refuses_dollar_prefixed_bindings() {
    // The oracle's `dollar_prefix_invalid`
    // (`phases/2-analyze/visitors/shared/utils.js:278` —
    // `node.name.startsWith('$')` on a Binding) is a Svelte reserved-prefix rule
    // on a BINDING, not on a reference and not a JS early error. `$$slots` is
    // the sharp edge: a *reference* is the real runtime value the transform
    // injects (`$.sanitize_slots`), so the guard exempts it, but a
    // *declaration* of that name is a compile error and must not inherit the
    // reference's exemption. Every case below was probe-verified oracle-rejected
    // against the pinned compiler.
    //
    // Variable declarators — any declaration kind, any function depth
    // (the `VariableDeclarator` visitor validates with no `function_depth`
    // argument, so the depth gate never applies), destructured or not.
    assert_unsupported(
        "<script>\n\tlet $$slots = 1;\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script module>\n\tlet $$slots = 1;\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tconst $$slots = 1;\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tfunction f() {\n\t\tvar $$slots = 1;\n\t\treturn $$slots;\n\t}\n</script>\n<p>{f()}</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tfunction a() {\n\t\tfunction b() {\n\t\t\tlet $$slots = 1;\n\t\t\treturn $$slots;\n\t\t}\n\t\treturn b();\n\t}\n</script>\n<p>{a()}</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tlet o = {};\n\tconst { $$slots } = o;\n</script>\n<p>x</p>",
        "$$slots",
    );
    // A function / class declaration id, a function-expression id, and a catch
    // parameter are bindings too.
    assert_unsupported(
        "<script>\n\tfunction $$slots() {}\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tclass $$slots {}\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tconst f = function $$slots() {};\n</script>\n<p>{f.name}</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tlet r = 0;\n\ttry {\n\t\tr = 1;\n\t} catch ($$slots) {\n\t\tr = 2;\n\t}\n</script>\n<p>{r}</p>",
        "$$slots",
    );
    // An import's local — the oracle's message names imports explicitly.
    assert_unsupported(
        "<script>\n\timport { $$slots } from './x.js';\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\timport $$slots from './x.js';\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\timport * as $$slots from './x.js';\n</script>\n<p>x</p>",
        "$$slots",
    );
    // The rule is the `$` prefix, not the name — the same positions refuse for
    // any `$`-prefixed binding. These cover the positions the pre-existing
    // reference-side refusal never reached (a class declaration id, an import
    // local), which over-accepted for EVERY name before the binding rule
    // existed, not only for `$$slots`.
    assert_unsupported("<script>\n\tclass $Foo {}\n</script>\n<p>x</p>", "$Foo");
    assert_unsupported(
        "<script>\n\timport { $foo } from './x.js';\n</script>\n<p>x</p>",
        "$foo",
    );
    assert_unsupported(
        "<script>\n\tfunction $$props() {}\n</script>\n<p>x</p>",
        "$$props",
    );
}

#[test]
fn compile_refuses_escaped_reserved_bindings() {
    // The crate's standing escaped-identifier residual, CLOSED. A `\u`-escaped
    // identifier decodes to a reserved name the oracle rejects (it reads the
    // DECODED `node.name`), but the name-extraction helpers were span-identity and
    // bailed on `escaped_name`, so every guarded position over-accepted. Each rule
    // now decodes via the interner (`Identifier::name`), matching the oracle.
    //
    // `dollar_prefix_invalid` across all six binding positions — `$` = `$`.
    for (source, name) in [
        // A declarator leaf (via the decode-aware pattern collector).
        (
            "<script>let \\u0024x = 1; \\u0024x;</script>\n<p>a</p>",
            "$x",
        ),
        // A function-declaration id.
        ("<script>function \\u0024f() {}</script>\n<p>a</p>", "$f"),
        // A class-declaration id.
        ("<script>class \\u0024C {}</script>\n<p>a</p>", "$C"),
        // A function-expression id.
        (
            "<script>const f = function \\u0024g() {};</script>\n<p>a</p>",
            "$g",
        ),
        // An import specifier's local.
        (
            "<script>import { \\u0024x } from './m.js'; \\u0024x;</script>\n<p>a</p>",
            "$x",
        ),
        // A catch-clause parameter (via the decode-aware pattern collector).
        (
            "<script>let r = 0; try { r = 1; } catch (\\u0024x) { r = 2; }</script>\n<p>{r}</p>",
            "$x",
        ),
    ] {
        assert_unsupported(source, name);
    }
    // `props_illegal_name`, declare-site — an escaped `$$` key (`$$x`).
    assert_unsupported(
        "<script>let { \\u0024\\u0024x: a } = $props(); a;</script>\n<p>{a}</p>",
        "prop name starting with `$$`",
    );
    // `props_illegal_name`, reference-site — an escaped `$$` property on a rest_prop.
    assert_unsupported(
        "<script>let { ...rest } = $props(); const x = rest.\\u0024\\u0024foo; x;</script>\n<p>x</p>",
        "prop name starting with `$$`",
    );
    // `invalid_arguments_usage` — an escaped `arguments` (`arguments`) reference
    // outside a function.
    assert_unsupported(
        "<script>const x = \\u0061rguments; x;</script>\n<p>x</p>",
        "arguments referenced outside a function",
    );
}

#[test]
fn compile_refuses_dollar_prefixed_binding_on_the_rewrite_path() {
    // `script_rewrite::rewrite_script_statement` rewrites a top-level instance-script
    // declaration instead of guard-walking it, so it needs the binding rule at
    // its OWN chokepoint — the guard walk never applies it here. Two halves,
    // both probe-verified oracle-rejected (`dollar_prefix_invalid`), both
    // over-accepted before the check moved ahead of the rune dispatch:
    //
    // (a) a non-rune declarator sharing a statement with a rune one. Its id took
    // the STORE-READ exemption, which this path's `WalkCtx` enables — so it
    // needed the base name (`x`) to be a binding, and any plain import supplies
    // that. Kept per rune kind: the trigger is the shared-declaration rune path,
    // and each kind reaches it.
    for init in ["$state(1)", "$state.raw(1)", "$props()", "$derived(1 + 1)"] {
        assert_unsupported(
            &format!(
                "<script>\n\timport {{ x }} from './s.js';\n\tlet a = {init}, $x = 2;\n</script>\n<p>{{$x}}</p>"
            ),
            "$x",
        );
    }
    // (b) the rune declarator's OWN id, which was not walked at all — so unlike
    // (a) it never depended on a bound base name, and no template read is needed
    // to reach it.
    for init in [
        "$state(1)",
        "$state.raw(1)",
        "$derived(1 + 1)",
        "$props.id()",
    ] {
        assert_unsupported(
            &format!("<script>\n\tlet $x = {init};\n</script>\n<p>hi</p>"),
            "$x",
        );
    }
}

#[test]
fn compile_refuses_dollar_prefixed_class_expression_id() {
    // A class EXPRESSION id is the one `$`-prefixed binding name the oracle
    // ACCEPTS (it declares no binding, so `dollar_prefix_invalid` never fires),
    // and tsv over-refuses it deliberately. Reproducing the oracle here means
    // reproducing two defects: its reference analysis is name-based and counts
    // the id as a READ, so `class $$slots {}` injects `$.sanitize_slots`, and
    // `class $Foo {}` drives its store rewrite to emit `class $.store_get(…) {}`
    // — invalid JS. Pinned so the over-refusal is a decision, not a drift.
    assert_unsupported(
        "<script>\n\tconst C = class $$slots {};\n</script>\n<p>{C.name}</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tconst C = class $Foo {};\n</script>\n<p>{C.name}</p>",
        "$Foo",
    );
}

#[test]
fn compile_refuses_dollar_prefixed_binding_the_oracle_exempts_by_depth() {
    // Two of the guarded positions are oracle-rejected only at the TOP LEVEL: a
    // function-expression id and a catch-clause parameter both declare through
    // `scope.js:695`, the one `validate_identifier_name` call path that passes
    // `function_depth`, and the instance script's top-level scope sits at depth
    // 1. So the oracle ACCEPTS these three inside a function body
    // (probe-verified against the pinned compiler) and tsv refuses them anyway.
    //
    // The over-refusal is deliberate, not a missing depth check. tsv's
    // `WalkCtx::fn_depth` is NOT the oracle's `function_depth`: the oracle's
    // non-porous increment happens at a function's `BlockStatement`
    // (`scope.js:1174-1188`), so an expression-bodied arrow does not increment
    // it and does increment tsv's. Gating on `fn_depth == 0` would compile the
    // oracle-REJECTED `const h = () => function $$slots() {}` — an
    // over-acceptance, which is a refusal-contract bug, where this is only an
    // over-refusal. Pinned so the trade is a decision, not a drift.
    for (source, name) in [
        (
            "<script>\n\tfunction f() {\n\t\ttry {\n\t\t\tnull;\n\t\t} catch ($$slots) {\n\t\t\treturn $$slots;\n\t\t}\n\t}\n</script>\n<p>{f()}</p>",
            "$$slots",
        ),
        (
            "<script>\n\tfunction f() {\n\t\treturn function $$slots() {};\n\t}\n</script>\n<p>{f().name}</p>",
            "$$slots",
        ),
        (
            "<script>\n\timport { x } from './store.js';\n\tfunction f() {\n\t\ttry {\n\t\t\tnull;\n\t\t} catch ($x) {\n\t\t\treturn $x;\n\t\t}\n\t}\n</script>\n<p>{f()}{$x}</p>",
            "$x",
        ),
    ] {
        assert_unsupported(source, name);
    }
}

#[test]
fn compile_allows_dollar_prefixed_binding_positions_the_oracle_exempts() {
    // The complement of the rule above, and the real risk of adding it: the
    // oracle EXEMPTS a `$`-prefixed name in a parameter position
    // (`declaration_kind` `param`/`rest_param`) and in a template binding
    // (declared in scopes past its `function_depth <= 1` gate), so over-refusing
    // there would break components the oracle compiles. Probe-verified
    // oracle-accepted; the `$$slots` cases also carry a genuine `$$slots`
    // reference, the cross the name-keyed carve-out used to conflate.
    //
    // ⚠️ Read the coverage narrowly: every case below is named `$$slots`, and
    // that is not a stylistic choice — it is the ONLY `$`-prefixed name this
    // test CAN cover. For any other name in these same oracle-exempt positions
    // (`function f($p)`, `{#each … as $x}`, `{@const $c = 1}`) tsv already
    // over-refuses, via the unrelated pre-existing reference-side store
    // refusal (`Refusal::DollarPrefixedIdentifier` — a `$`-prefixed read whose
    // base is not a binding), which fires long before any binding rule. That
    // over-refusal is pre-existing and contract-safe (refusing is never a
    // refusal-contract bug), but it means these seven cases pin the exemption
    // only for the one name the reference carve-out already lets through. They
    // are NOT evidence that the binding rule leaves the oracle's exempt
    // positions generally intact — nothing here can be, until that store
    // refusal narrows.
    for source in [
        "<script>\n\tfunction f($$slots) {\n\t\treturn $$slots;\n\t}\n</script>\n{#if $$slots.a}<p>{f(1)}</p>{/if}",
        "<script>\n\tconst g = ($$slots) => $$slots;\n</script>\n<p>{g(1)}</p>",
        "<script>\n\tfunction f(...$$slots) {\n\t\treturn $$slots.length;\n\t}\n</script>\n<p>{f(1)}</p>",
        "{#each [1] as n}{@const $$slots = n}<p>{$$slots}</p>{/each}",
        "{#each [1] as $$slots}<p>{$$slots}</p>{/each}",
        "{#await Promise.resolve(1) then $$slots}<p>{$$slots}</p>{/await}",
        "{#snippet s($$slots)}<p>{$$slots}</p>{/snippet}",
    ] {
        let _ = compile_js(source);
    }
}

#[test]
fn compile_allows_dollar_member_names() {
    // A `$`-prefixed *name* (non-computed member property) is not a rune
    // reference — it stays compilable. The member access roots in the prop
    // `a`, so `needs_context` wraps the body. Full-string equality (not a
    // substring check) so the wrapper can't silently regress.
    let js = compile_js("<script>let { a } = $props();</script>\n<p>{a.$foo}</p>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \t$$renderer.component(($$renderer) => {\n\
             \t\tlet { a } = $$props;\n\
             \t\t$$renderer.push(`<p>${$.escape(a.$foo)}</p>`);\n\
             \t});\n\
             }\n"
    );
}

#[test]
fn compile_refuses_props_illegal_name_declare_site() {
    // The oracle's `props_illegal_name` (VariableDeclarator.js:94-103): a `$props()`
    // destructure property whose non-computed Identifier key starts with `$$` is
    // reserved for Svelte internals and rejected. Reaches the check even without a
    // rest/bindable (the plain `{ $$slots: a }` form).
    assert_unsupported(
        "<script>let { $$slots: a } = $props(); a;</script>\n<p>{a}</p>",
        "prop name starting with `$$`",
    );
    assert_unsupported(
        "<script>let { $$foo: a } = $props(); a;</script>\n<p>{a}</p>",
        "prop name starting with `$$`",
    );
    // Mixed with a normal prop — still caught.
    assert_unsupported(
        "<script>let { a, $$foo: b } = $props(); a; b;</script>\n<p>{a}{b}</p>",
        "prop name starting with `$$`",
    );
    // ⭐ No reason-stealing: a `$$`-prefixed BINDING (shorthand / default) declares a
    // `$$` binding, refused UPSTREAM as `DollarPrefixedBinding` (the oracle's
    // `dollar_prefix_invalid`, which fires first) — NOT props_illegal_name.
    assert_unsupported(
        "<script>let { $$foo } = $props(); $$foo;</script>\n<p>x</p>",
        "$-prefixed binding $$foo",
    );
    assert_unsupported(
        "<script>let { $$foo = 1 } = $props(); $$foo;</script>\n<p>x</p>",
        "$-prefixed binding $$foo",
    );
    // Discriminating controls, all COMPILE: a single-`$` key (not `$$`), a plain key,
    // a string-literal `$$` key (the oracle's check is Identifier-only), and a plain
    // rest destructure.
    let _ = compile_js("<script>let { $foo: a } = $props(); a;</script>\n<p>{a}</p>");
    let _ = compile_js("<script>let { foo: a } = $props(); a;</script>\n<p>{a}</p>");
    let _ = compile_js("<script>let { '$$slots': a } = $props(); a;</script>\n<p>{a}</p>");
    let _ = compile_js("<script>let { a, ...rest } = $props(); a; rest;</script>\n<p>{a}</p>");
}

#[test]
fn compile_refuses_props_invalid_pattern() {
    // The oracle's `props_invalid_pattern` (VariableDeclarator.js:97-110): a `$props()`
    // ObjectPattern property that is COMPUTED or whose value (after stripping a default)
    // is not a plain Identifier. The three per-property checks fire in source order,
    // first-wins (`e.*` throws): computed → props_invalid_pattern, then a `$$` key →
    // props_illegal_name, then a non-Identifier value → props_invalid_pattern.
    let reason = "$props() destructure with a computed key or nested pattern";
    // Computed key.
    assert_unsupported(
        "<script>const x = 'k'; let { [x]: a } = $props(); a;</script>\n<p>{a}</p>",
        reason,
    );
    // Computed key with a rest present (does not early-return; caught before the rewrite).
    assert_unsupported(
        "<script>const x = 'k'; let { [x]: a, ...rest } = $props(); a; rest;</script>\n<p>{a}</p>",
        reason,
    );
    // Nested object-pattern value.
    assert_unsupported(
        "<script>let { a: { b } } = $props(); b;</script>\n<p>{b}</p>",
        reason,
    );
    // Nested array-pattern value.
    assert_unsupported(
        "<script>let { a: [b] } = $props(); b;</script>\n<p>{b}</p>",
        reason,
    );
    // Nested value carrying a default (the default is stripped, its `left` is the pattern).
    assert_unsupported(
        "<script>let { a: { b } = {} } = $props(); b;</script>\n<p>{b}</p>",
        reason,
    );
    // Nested value with a `$bindable()` default — the value (default-stripped) is still an
    // ObjectPattern, so props_invalid_pattern fires FIRST (before the guard sees $bindable).
    assert_unsupported(
        "<script>let { a: { b } = $bindable() } = $props(); b;</script>\n<p>{b}</p>",
        reason,
    );
    // ⭐ Source-order first-wins. A computed key BEFORE a `$$` key → the computed check
    // wins (props_invalid_pattern), matching the oracle.
    assert_unsupported(
        "<script>const k = 'z'; let { [k]: a, $$foo: b } = $props(); a; b;</script>\n<p>{a}{b}</p>",
        reason,
    );
    // …but a `$$` key BEFORE a computed key → props_illegal_name wins (source order).
    assert_unsupported(
        "<script>const k = 'z'; let { $$foo: b, [k]: a } = $props(); a; b;</script>\n<p>{a}{b}</p>",
        "prop name starting with `$$`",
    );
    // Discriminating controls, all COMPILE: plain, renamed, default, rest, and bindable
    // (both plain and renamed) — every value is a plain Identifier after default-strip.
    let _ = compile_js("<script>let { a, b } = $props(); a; b;</script>\n<p>{a}{b}</p>");
    let _ = compile_js("<script>let { a: b } = $props(); b;</script>\n<p>{b}</p>");
    let _ = compile_js("<script>let { a = 1 } = $props(); a;</script>\n<p>{a}</p>");
    let _ = compile_js("<script>let { a, ...rest } = $props(); a; rest;</script>\n<p>{a}</p>");
    let _ = compile_js(
        "<script>let { value = $bindable() } = $props(); value;</script>\n<p>{value}</p>",
    );
    let _ =
        compile_js("<script>let { value: v = $bindable(1) } = $props(); v;</script>\n<p>{v}</p>");
}

#[test]
fn compile_refuses_props_illegal_name_member_site() {
    // The oracle's `props_illegal_name` REFERENCE-site rule
    // (`MemberExpression.js:11-16`): a non-computed `.$$…` access on a plain
    // Identifier bound to a `$props()` rest_prop. A `rest_prop` arises from the
    // whole-object `let props = $props()` (`VariableDeclarator.js:87-90`) and from
    // the REST element of `let { a, ...rest } = $props()` (`:46-47`); the NAMED
    // props are `prop`, never `rest_prop`.
    for source in [
        // The three declaration forms: destructure rest, whole-object, mixed.
        "<script>let { ...rest } = $props(); const x = rest.$$slots; x;</script>\n<p>x</p>",
        "<script>let props = $props(); const x = props.$$slots; x;</script>\n<p>x</p>",
        "<script>let { a, ...rest } = $props(); a; const x = rest.$$slots; x;</script>\n<p>x</p>",
        // Nested member — the INNER `rest.$$foo` fires (its object is the
        // Identifier `rest`; the outer `.bar`'s object is a MemberExpression).
        "<script>let { ...rest } = $props(); const x = rest.$$foo.bar; x;</script>\n<p>x</p>",
        // Template position.
        "<script>let { ...rest } = $props();</script>\n{rest.$$slots}",
        // Optional chain: tsv's internal AST drops `ChainExpression`, so `rest?.$$foo`
        // is a plain `MemberExpression{optional:true}` — the arm does NOT gate on
        // `optional`, matching the oracle (which has no such gate).
        "<script>let { ...rest } = $props(); const x = rest?.$$foo; x;</script>\n<p>x</p>",
        // Bare `$$` property (`\"$$\".starts_with(\"$$\")` is true).
        "<script>let { ...rest } = $props(); const x = rest.$$; x;</script>\n<p>x</p>",
        // Snippet body — the walk descends into `{#snippet}` bodies.
        "<script>let { ...rest } = $props();</script>\n{#snippet s()}{rest.$$foo}{/snippet}",
        // Script arrow body.
        "<script>let { ...rest } = $props(); const f = () => rest.$$foo; f;</script>\n<p>x</p>",
        // Dropped event handler — the walk reaches dropped handlers.
        "<script>let { ...rest } = $props();</script>\n<button onclick={() => rest.$$foo}>x</button>",
        // Dropped `{:catch}` — the `in_dropped_catch` walk.
        "<script>let { ...rest } = $props();</script>\n{#await p}a{:catch e}{rest.$$foo}{/await}",
        // ⭐ Computed IDENTIFIER key `rest[$$slots]` / `props[$$slots]` — the
        // oracle's condition is `node.property.type === 'Identifier'` (NO computed
        // gate), so a computed identifier key matches. This is the case a `!computed`
        // gate would LEAK: `$$slots` is exempt from tsv's own `$$`-ref rule
        // (`rune_guard.rs`, the sanitize_slots ref), so nothing else would fire.
        "<script>let { ...rest } = $props(); const x = rest[$$slots]; x;</script>\n<p>x</p>",
        "<script>let props = $props(); const x = props[$$slots]; x;</script>\n<p>x</p>",
    ] {
        assert_unsupported(source, "prop name starting with `$$`");
    }
}

#[test]
fn compile_allows_member_access_that_is_not_rest_prop_illegal() {
    // Controls that MUST keep compiling — the member-site rule must not over-refuse.
    // A computed STRING key: the property is a Literal, not an Identifier, so the
    // `Expression::Identifier(prop)` arm fails and it never matches — the oracle
    // also compiles it. (Contrast the computed IDENTIFIER key `rest[$$slots]`,
    // which DOES match and refuses — in the refuse test above.)
    let _ = compile_js(
        "<script>let { ...rest } = $props(); const x = rest['$$slots']; x;</script>\n<p>x</p>",
    );
    // A NAMED prop (`a` is `prop`, not `rest_prop`) — `a.$$foo` is legal; it wraps
    // in `$$renderer.component` (prop-rooted member), matching the oracle.
    let _ = compile_js("<script>let { a } = $props(); const x = a.$$foo; x;</script>\n<p>x</p>");
    // A non-`$$` property on a rest_prop.
    let _ =
        compile_js("<script>let { ...rest } = $props(); const x = rest.foo; x;</script>\n<p>x</p>");
    // A plain object, not a props binding.
    let _ = compile_js("<script>let o = {}; const x = o.$$foo; x;</script>\n<p>x</p>");
}
