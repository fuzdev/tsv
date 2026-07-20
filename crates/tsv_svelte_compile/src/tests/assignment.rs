//! The `validate_assignment` family: const / each-item / snippet-parameter.

use super::support::*;

#[test]
fn compile_constant_assignment_refuses() {
    // `validate_no_const_assignment` (`shared/utils.js:84`): a write to a `const`
    // declarator or an import local is `constant_assignment`, keyed on the
    // DECLARATION KEYWORD — so a reactive `const a = $state(0)` is refused too.
    // Both operator forms reach it (`AssignmentExpression.js:11`,
    // `UpdateExpression.js:11`).
    assert_unsupported("<script>const a = c(); a += 1;</script>", "a constant");
    assert_unsupported("<script>const a = c(); a++;</script>", "a constant");
    assert_unsupported(
        "<script>const a = $state(0); a += 1;</script>",
        "a constant",
    );
    assert_unsupported("<script>const a = $state(0); a++;</script>", "a constant");
    assert_unsupported(
        "<script>import {a} from './m.js'; a = 1;</script>",
        "a constant",
    );
    // A module-script const is unassignable on the same terms.
    assert_unsupported(
        "<script module>const a = 1;</script><script>a = 2;</script>",
        "a constant",
    );
    // Inside a dropped event handler too — the oracle validates in phase 2,
    // before it decides what SSR emits.
    assert_unsupported(
        "<script>const a = $state(0);</script><button onclick={() => a++}>x</button>",
        "a constant",
    );
    // Destructuring assignment: the rule recurses through ArrayPattern elements
    // and ObjectPattern property VALUES (`validator/samples/assignment-to-const-5`
    // and `-7`).
    assert_unsupported(
        "<script>const arr = [1, 2]; [arr, arr[1]] = [arr[1], arr[0]];</script>",
        "a constant",
    );
    assert_unsupported(
        "<script>const arr = [1]; ({a: {arr}} = x);</script>",
        "a constant",
    );
}

#[test]
fn compile_shadowing_js_local_assignment_compiles() {
    // The oracle resolves an assignment target through its SCOPE CHAIN
    // (`phases/scope.js`), so a nested re-declaration shadows a component-level
    // `const`/import and the write is to the local — not `constant_assignment`.
    // The shape is ordinary enough to appear in Svelte's own suite
    // (`runtime-runes/samples/mutation-local`).
    //
    // A function parameter shadowing a component `const`.
    let _ = compile_js("<script>const x = 1; function f(x) { x = 2; return x; }</script>");
    // A function-body `let`, written from a nested block — the suite's shape.
    let _ = compile_js(
        "<script>function f(i) { let x = i; if (x > 0) { x = 2; } return x; }\
         const x = f(1);</script>",
    );
    // An arrow parameter shadowing an IMPORT local.
    let _ = compile_js("<script>import {a} from './m.js'; const g = (a) => { a = 1; };</script>");
    // A plain nested block `let`.
    let _ = compile_js("<script>const b = 1; function f() { { let b = 0; b = 2; } }</script>");
    // A `catch` parameter, and a `for` head binding.
    let _ = compile_js(
        "<script>const e = 1; function f() { try { g(); } catch (e) { e = null; } }</script>",
    );
    let _ =
        compile_js("<script>const i = 1; function f() { for (let i = 0; i < 2; i++) {} }</script>");
}

#[test]
fn compile_shadow_scope_does_not_leak() {
    // ⚠️ The UNSAFE direction: a shadow that outlives its scope would stop
    // refusing a genuine `constant_assignment`. Each of these writes resolves to
    // the component-level binding, so each must still refuse.
    //
    // A block-scoped `let` must not leak to the statements after its block.
    assert_unsupported(
        "<script>const a = 1; function f() { { let a = 0; } a = 2; }</script>",
        "a constant",
    );
    // A function's parameters must not leak past the function.
    assert_unsupported(
        "<script>const a = 1; function f(a) { return a; } a = 2;</script>",
        "a constant",
    );
    // Nor must a sibling function's locals reach another function.
    assert_unsupported(
        "<script>const a = 1; function f() { let a = 0; } function g() { a = 2; }</script>",
        "a constant",
    );
    // A `for` head binding must not leak past the loop.
    assert_unsupported(
        "<script>const a = 1; function f() { for (let a = 0; a < 2; a++) {} a = 3; }</script>",
        "a constant",
    );
    // A `catch` parameter must not leak past its handler.
    assert_unsupported(
        "<script>const a = 1; function f() { try { g(); } catch (a) {} a = 2; }</script>",
        "a constant",
    );
    // And a TEMPLATE-scoped binding must keep its own rule: the `{#each}` item and
    // the `{#snippet}` parameter are declared through the same walk, so neither may
    // be absorbed into the JS-local set (which suppresses refusal).
    assert_unsupported(
        "<script>let arr = $state([1]);</script>\
         {#each arr as value}<button onclick={() => value = 1}>x</button>{/each}",
        "an {#each} item",
    );
    assert_unsupported(
        "{#snippet s(p)}<button onclick={() => p = 1}>x</button>{/snippet}{@render s(1)}",
        "a {#snippet} parameter",
    );
}

#[test]
fn compile_each_item_assignment_refuses() {
    // `validate_assignment` (`shared/utils.js:33`): a write to an `{#each}`
    // context binding is `each_item_invalid_assignment` in runes mode, which this
    // compiler unconditionally is. Reached from an assignment, an update, and a
    // `bind:` alike (`BindDirective.js:181`).
    assert_unsupported(
        "<script>let arr = $state([1]);</script>\
         {#each arr as value}<button onclick={() => value += 1}>x</button>{/each}",
        "an {#each} item",
    );
    assert_unsupported(
        "<script>let arr = $state([1]);</script>\
         {#each arr as value}<button onclick={() => value++}>x</button>{/each}",
        "an {#each} item",
    );
    assert_unsupported(
        "<script>let arr = $state([{element: null}]);</script>\
         {#each arr as {element}}<input bind:this={element} />{/each}",
        "an {#each} item",
    );
    // The fallback shares the block's scope (`scope.js:1280`).
    assert_unsupported(
        "<script>let arr = $state([1]);</script>\
         {#each arr as value}<p>a</p>{:else}<button onclick={() => value = 1}>x</button>{/each}",
        "an {#each} item",
    );
}

#[test]
fn compile_template_scoped_const_assignment_refuses() {
    // The TEMPLATE-scoped consts: a `{@const}` name (`phases/scope.js:1205`, whose
    // `declaration_kind` is the `VariableDeclaration`'s own `const`), a
    // `{:then}`/`{:catch}` value (`:1310`/`:1324`) and an `{#each}` INDEX
    // (`:1273`). All three are `declaration_kind: 'const'` to the oracle, so a
    // write to one is `constant_assignment` — live-verified, the oracle rejects
    // every case below with `Cannot assign to constant`.
    //
    // ⚠️ These are OVER-ACCEPTANCES when unrecorded, not over-refusals: the names
    // are purely template-local, so nothing falls through to a component-level set
    // and no rule fires at all.
    //
    // ⚠️ Two of the three write POSITIONS below are load-bearing. An assignment
    // sitting directly in an emitted template expression (`{(c = 2)}`) also trips
    // `mutation inside a template expression`, an unrelated general rule that fires
    // whatever the target is (verified: a plain `let` write there refuses too,
    // while the oracle ACCEPTS it) — so that position MASKS this rule and has
    // already been mistaken for a refutation. The event-handler arrow and the
    // dropped `{:catch}` are the unmasked positions, and each form is covered in
    // both.
    // Position 1: an event-handler arrow — dropped from SSR, still validated in
    // phase 2.
    assert_unsupported(
        "{#if true}{@const c = 1}<button onclick={() => (c = 2)}>x</button>{/if}",
        "a constant",
    );
    assert_unsupported(
        "<script>let p = Promise.resolve(1);</script>\
         {#await p then v}<button onclick={() => (v = 2)}>x</button>{/await}",
        "a constant",
    );
    assert_unsupported(
        "<script>let xs = [1];</script>\
         {#each xs as x, i}<button onclick={() => (i = 2)}>{x}</button>{/each}",
        "a constant",
    );
    // The same three forms written inside a dropped `{:catch}`, the second
    // unmasked position: the branch never reaches SSR emission, so no emission
    // refusal can fire there and the write is graded by this rule alone.
    assert_unsupported(
        "<script>let p = Promise.resolve(1);</script>\
         {#await p}<i>w</i>{:catch e}{@const c = 1}{(c = 2)}{/await}",
        "a constant",
    );
    assert_unsupported(
        "<script>let p = Promise.resolve(1);</script>\
         {#await p}<i>w</i>{:catch e}{(e = 2)}{/await}",
        "a constant",
    );
    assert_unsupported(
        "<script>let p = Promise.resolve(1); let xs = [1];</script>\
         {#await p}<i>w</i>{:catch e}{#each xs as x, i}{(i = 2)}{x}{/each}{/await}",
        "a constant",
    );
    // The oracle's scope PRE-PASS again (see
    // `compile_nested_const_before_its_declaration_refuses`): a `{@const}` is
    // declared into its fragment's scope before any node of that fragment is
    // visited, so a write textually EARLIER still resolves to it. Live-verified.
    assert_unsupported(
        "{#if true}<button onclick={() => (c = 2)}>x</button>{@const c = 1}{/if}",
        "a constant",
    );
    // A destructured `{@const}` binds each name the pattern declares.
    assert_unsupported(
        "<script>let o = {a: 1};</script>\
         {#if true}{@const {a} = o}<button onclick={() => (a = 2)}>x</button>{/if}",
        "a constant",
    );
}

#[test]
fn compile_each_index_is_not_an_each_item() {
    // ⭐ The two bindings of ONE construct take DIFFERENT oracle rules, and
    // conflating them is a bug in either direction. In `{#each xs as x, i}` the
    // ITEM is declared `('each', 'const')` (`phases/scope.js:1244`) and
    // `validate_no_const_assignment` EXCLUDES `kind === 'each'`, so it carries
    // `each_item_invalid_assignment`; the INDEX is `('template' | 'static',
    // 'const')` (`:1273`) and carries `constant_assignment`. Both live-verified
    // against the oracle, which reports "Cannot reassign or bind to each block
    // argument" for the first and "Cannot assign to constant" for the second.
    assert_unsupported(
        "<script>let xs = [1];</script>\
         {#each xs as x, i}<button onclick={() => (x = 2)}>{i}</button>{/each}",
        "an {#each} item",
    );
    assert_unsupported(
        "<script>let xs = [1];</script>\
         {#each xs as x, i}<button onclick={() => (i = 2)}>{x}</button>{/each}",
        "a constant",
    );
}

#[test]
fn compile_template_scoped_const_boundary_accepts() {
    // The acceptance half, so the new template-const scope cannot widen into an
    // over-refusal. Each of these the oracle COMPILES (live-verified).
    //
    // A JS binding nests INSIDE the template scope, so a handler parameter of the
    // same name shadows the `{@const}` and the write is to the parameter — which
    // is why `js_scope` is consulted before the template-const set.
    let _ = compile_js("{#if true}{@const c = 1}<button onclick={(c) => (c = 2)}>x</button>{/if}");
    // A MEMBER target writes THROUGH the binding, matching no branch of the
    // oracle's validator.
    let _ = compile_js(
        "<script>let o = {v: 1};</script>\
         {#if true}{@const c = o}<button onclick={() => (c.v = 2)}>x</button>{/if}",
    );
    // The scopes END at their block: an `{#each}` index is out of scope after the
    // block, a `{:then}` value is not in scope in the `{:catch}` branch, and a
    // `{@const}` is confined to its own fragment.
    let _ = compile_js(
        "<script>let xs = [1];</script>\
         {#each xs as x, idx}{x}{/each}<button onclick={() => (idx = 2)}>x</button>",
    );
    let _ = compile_js(
        "<script>let p = Promise.resolve(1); let v = $state(0);</script>\
         {#await p then v}{v}{:catch e}<button onclick={() => (v = 2)}>x</button>{/await}",
    );
    let _ = compile_js(
        "<script>let c = $state(0);</script>\
         {#if true}{@const c = 1}{c}{/if}<button onclick={() => (c = 2)}>x</button>",
    );
}

#[test]
fn compile_snippet_parameter_assignment_refuses() {
    // `validate_assignment` (`shared/utils.js:37`): a write to a `{#snippet}`
    // parameter is `snippet_parameter_assignment` — and, unlike the each rule,
    // NOT gated on runes mode.
    assert_unsupported(
        "{#snippet foo(value)}<button onclick={() => value += 1}>x</button>{/snippet}",
        "a {#snippet} parameter",
    );
}

#[test]
fn compile_invalid_assignment_boundary_accepts() {
    // The acceptance half of the rule, so a widening over-refusal is caught.
    //
    // A MEMBER target writes THROUGH the binding and never rebinds it — it
    // matches no branch of the oracle's validator.
    let _ = compile_js(
        "<script>const o = $state({v: 1});</script><button onclick={() => o.v = 2}>x</button>",
    );
    let _ = compile_js(
        "<script>let arr = $state([{v: 1}]);</script>\
         {#each arr as item}<button onclick={() => item.v = 2}>x</button>{/each}",
    );
    // A `let` is not a constant, and the block scopes END at their block.
    let _ = compile_js("<script>let a = $state(0);</script><button onclick={() => a++}>x</button>");
    let _ = compile_js(
        "<script>let a = $state(0); let arr = $state([1]);</script>\
         {#each arr as a}<p>{a}</p>{/each}<button onclick={() => a++}>x</button>",
    );
    let _ = compile_js(
        "{#snippet foo(v)}<p>{v}</p>{/snippet}<script>let v = $state(0);</script>\
         <button onclick={() => v++}>x</button>",
    );
    // The pattern recursion stops exactly where the oracle's does: a RestElement
    // and an AssignmentPattern match no branch, so a const there is accepted.
    let _ = compile_js(
        "<script>const a = 1;</script><button onclick={() => { [...a] = x; }}>x</button>",
    );
}

#[test]
fn compile_nested_const_assignment_refuses() {
    // `validate_no_const_assignment` reads the SCOPE CHAIN, so its `const` test is
    // not confined to script scope: a NESTED `const` — a block-local, a function
    // body local, a `for (const … of …)` head — is `declaration_kind: 'const'` to
    // the oracle exactly as a top-level one is, and a write to it is
    // `constant_assignment`. Live-verified against the oracle: each of these is
    // rejected with `Cannot assign to constant`.
    //
    // A nested `const` colliding with a component-level `const`/import must refuse
    // as the INNER binding's rule, not fall through to the outer name's.
    assert_unsupported(
        "<script>const a = 1; function f() { { const a = 0; a = 2; } }</script>",
        "a constant",
    );
    assert_unsupported(
        "<script>const a = 1; function f() { for (const a of xs) { a = 2; } }</script>",
        "a constant",
    );
    assert_unsupported(
        "<script>const a = 1; function f() { const a = 0; a = 2; }</script>",
        "a constant",
    );
    assert_unsupported(
        "<script>import {a} from './m.js'; function f() { const a = 0; a = 2; }</script>",
        "a constant",
    );
    // And with NO collision at all — the nested `const` is the only binding of the
    // name, so nothing but its own scope entry can refuse it.
    assert_unsupported(
        "<script>const b = 1; function f() { const a = 0; a = 2; }</script>",
        "a constant",
    );
    // The template reaches the same walk.
    assert_unsupported(
        "<script>const b = 1;</script>\
         <button onclick={() => { const a = 0; a = 2; }}>x</button>",
        "a constant",
    );
}

#[test]
fn compile_innermost_js_binding_wins() {
    // ⚠️ The discriminating control on the nested-`const` rule: a `let` nested
    // INSIDE a `const` of the same name is the innermost binding, so the write is
    // to the `let` and nothing refuses. Live-verified: the oracle COMPILES this.
    // A rule that merely asked "is this name const anywhere open?" would refuse it.
    let _ = compile_js(
        "<script>const b = 1; function f() { const a = 1; { let a = 2; a = 3; } }</script>",
    );
    // The mirror ordering (a `const` inside a `let`) still refuses — same shape,
    // opposite innermost kind, so the two together pin that the ORDER is what is
    // read, not the mere presence of a `const`.
    assert_unsupported(
        "<script>const b = 1; function f() { let a = 1; { const a = 2; a = 3; } }</script>",
        "a constant",
    );
    // A nested `const`'s scope must not outlive it: after the block closes the
    // write resolves to the enclosing `let` again and compiles.
    let _ = compile_js(
        "<script>const b = 1; function f() { let a = 1; { const a = 2; } a = 3; }</script>",
    );
}

#[test]
fn compile_nested_const_before_its_declaration_refuses() {
    // The oracle builds its scopes in a PRE-PASS (`phases/scope.js`'s
    // `create_scopes`), so every binding of a block exists before any reference in
    // it is validated. A write textually EARLIER than the `const` therefore still
    // resolves to it and is `constant_assignment` — a TDZ error at runtime, but a
    // compile error to Svelte either way. Live-verified: the oracle rejects this
    // with `Cannot assign to constant`.
    //
    // ⚠️ It is an OVER-ACCEPTANCE, not an over-refusal, when the walk misses it:
    // the name is purely LOCAL, so leaving it unrecorded yields no rule at all
    // rather than falling through to a component-level one.
    assert_unsupported(
        "<script>function g() { z = 1; const z = 2; return z; }</script>",
        "a constant",
    );
    // With a component-level collision the fall-through WOULD have caught it —
    // the control that shows the local-only case above is the load-bearing one.
    assert_unsupported(
        "<script>const z = 0; function g() { z = 1; const z = 2; return z; }</script>",
        "a constant",
    );
    // The hoist is `const`-only and block-scoped, so it must not reach past its
    // block: after the block closes the write resolves to the enclosing `let`.
    let _ = compile_js(
        "<script>const b = 1; function f() { let a = 1; a = 3; { const a = 2; } }</script>",
    );
}

#[test]
fn compile_switch_case_const_shares_one_scope() {
    // `phases/scope.js:1173` — `SwitchStatement: create_block_scope`. The oracle
    // gives a `switch` ONE block scope shared by every case, so a `const` in one
    // case is in scope for a write in another and refuses. Live-verified: the
    // oracle rejects both orderings with `Cannot assign to constant`.
    //
    // ⚠️ Also an OVER-ACCEPTANCE when missed — `w` is purely local.
    assert_unsupported(
        "<script>function f(v) { switch (v) { case 1: const w = 1; break; case 2: w = 2; } }</script>",
        "a constant",
    );
    // The reverse ordering (write first) needs the hoist as well as the shared
    // scope, so it pins both halves of the fix at once.
    assert_unsupported(
        "<script>function f(v) { switch (v) { case 1: w = 2; break; case 2: const w = 1; } }</script>",
        "a constant",
    );
    // The switch scope must not outlive the statement.
    let _ = compile_js(
        "<script>let b = 1; function f(v) { switch (v) { case 1: const w = 1; } w = 2; }</script>",
    );
}

#[test]
fn compile_each_item_assignment_refuses_the_checklist_repro() {
    // The exact repro `docs/checklist_svelte_compiler.md` carried as an open
    // over-acceptance until this rule landed — pinned so the doc's claim that the
    // row is closed cannot silently rot.
    assert_unsupported(
        "<script>let b = 1;</script>{#each [0] as b}<button onclick={() => { b++; }}>x</button>{/each}",
        "an {#each} item",
    );
}
