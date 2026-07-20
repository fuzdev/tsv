# Svelte Compiler Support (tsv_svelte_compile)

Coverage map for tsv's Svelte-to-JS compiler (`crates/tsv_svelte_compile`): which component shapes compile at oracle parity, which refuse, and which are planned. Companion to the parser/formatter checklists ([checklist_svelte.md](./checklist_svelte.md), [checklist_typescript.md](./checklist_typescript.md), [checklist_css.md](./checklist_css.md)).

## Coverage

The compiler targets **server (SSR) output for runes-mode components**, measured against Svelte's own `compile()` (pinned at **svelte 5.56.4**, the sidecar pin) as the correctness oracle. Parity is judged on the **canonical reprint** of both sides' JS (`canonicalize_js` — an intent-erased reprint, so a byte difference is a real code difference), plus byte-equal CSS.

**The refusal contract**: every component shape is exactly one of

- **Supported** — compiles, and the canonical form matches the oracle's byte-for-byte;
- **Refused** — `compile` returns `CompileError::Unsupported(Refusal)`, a typed refusal from the inventory in `crates/tsv_svelte_compile/src/refusal.rs`, never guessed output;
- **a bug** — both sides compile and the canonical forms differ (`compile_corpus_compare`'s MISMATCH bucket), generated JS fails its reparse self-validation (`CompileError::CorruptOutput`), or a TypeScript-only node survives type erasure (`CompileError::TypeErasureLeak`).

Inputs the oracle itself rejects (legacy-mode syntax, invalid JS, TypeScript in a plain script) are out of scope for parity — the corpus runner buckets them ORACLE_REJECTED.

A **Refused** entry below opens with the `Refusal`'s stable **bucket key** in a code span (user-chosen identifiers shown as `{name}`), so it can be matched directly against a `compile_corpus_compare` run, which reports those keys via `Refusal::bucket_key`. `compile_conformance_audit` gates that correspondence in one direction: a quoted key no variant produces fails the audit. The reverse does not hold — this is a coverage map, not a key dump, and a variant covered by a prose paragraph rather than its own bullet is fine (the audit reports the unquoted count without gating it). Each variant also carries a human-readable `Display` message that substitutes the real name and often adds a parenthetical the key omits — the two are deliberately decoupled so a message can be reworded without shifting corpus buckets.

**Verification**: `cargo run -p tsv_debug compile_fixtures_validate` (fixture parity, oracle freshness, canonicalize fixed points) and `deno task compile:corpus:compare` (corpus-scale bucketing over real repos + the Svelte test suites).

**Spec References**:

- Compiler source (the rule ledger): `../../svelte/packages/svelte/src/compiler/`
  - Analysis (phase 2): `../../svelte/packages/svelte/src/compiler/phases/2-analyze/`
  - Server transform (phase 3): `../../svelte/packages/svelte/src/compiler/phases/3-transform/server/`
- Svelte docs: `../../svelte/documentation/docs/`
- Compile fixtures: `tests/fixtures_compile/` (oracle-generated via `compile_fixture_init`)

Svelte-source line anchors below are valid at the 5.56.4 pin.

---

## Server-Shell Semantics

The module scaffold and the component-function shell, ported from Svelte's server transform.

### Module scaffold — Supported

- `import * as $ from 'svelte/internal/server';` first, then hoisted instance imports, then the exported component function (`transform-server.js:101`, `transform-server.js:303`).
- **Import hoisting**: instance-script `import` declarations hoist to module scope in source order — the oracle replaces each with an empty statement inside the component and pushes it to the hoisted block (`transform-server.js:123-126`). An import inside the component function would be invalid JS.

### `needs_context` — the `$$renderer.component(…)` wrapper

The oracle wraps the whole component body in `$$renderer.component(($$renderer) => { … })` when `should_inject_context = dev || analysis.needs_context` (`transform-server.js:260-272`). Phase 2 sets `needs_context`, monotonically, from five triggers:

| Trigger | Oracle site |
| --- | --- |
| any `new` expression (unconditional) | `2-analyze/visitors/NewExpression.js:14` |
| a member access whose root is unsafe | `2-analyze/visitors/MemberExpression.js:23-24` |
| a plain (non-rune) call whose callee root is unsafe | `2-analyze/visitors/CallExpression.js:31-33` |
| `$bindable` | `2-analyze/visitors/CallExpression.js:55` |
| `$effect` / `$effect.pre` | `2-analyze/visitors/CallExpression.js:149-150` |

**`is_safe_identifier` rule** (`2-analyze/visitors/shared/utils.js:175-194`): walk a member chain down `.object` to its root; a non-identifier root is unsafe; an identifier root is unsafe when its binding's `declaration_kind` is `import` or its `kind` is `prop`/`bindable_prop`/`rest_prop`. A plain local, a global (no binding), and rune bindings (`state`, `derived`, …) are safe.

tsv ports this as `needs_context.rs`, folding props + imports into a name set. `$effect` forces the wrapper through its own dropped-statement path; a `$bindable` prop forces it through the collected bindable set (see the `$bindable` section under Runes). Because the port is name-based where the oracle is scope-sensitive, two shapes can't be classified and refuse:

- **Refused**: `` member/call rooted at prop/import `{name}` also bound in a nested scope `` — the `needs_context` classification is ambiguous.
- **Refused**: `member/call rooted at an escaped identifier (classification not ported)` — the root's name can't be read from its raw span.

### `$$props` coupling — Supported

The component function signature is `($$renderer)` or `($$renderer, $$props)`: the oracle injects the `$$props` parameter when `should_inject_context` fires or the component declares props (`should_inject_props`, `transform-server.js:313-326`). tsv reproduces both the wrapper and the parameter coupling.

### Multi-declarator split — Supported

A multi-declarator **top-level instance declaration** splits into one declaration per declarator, source order preserved — the oracle's "one declarator per declaration" normalization (`2-analyze/index.js:1148`, `2-analyze/index.js:1154`). Nested declarations (function bodies, `for` heads) stay joined.

- **Refused**: `comments in a script alongside a multi-declarator declaration (the oracle re-anchors comments inside the split)`

### `$props()` rest injection — Supported

A rest element in the `$props()` object pattern gains `$$slots, $$events` immediately before it; a non-destructured `let props = $props()` becomes `let { $$slots, $$events, ...props } = $$props` (`3-transform/server/visitors/VariableDeclaration.js:60-77`). A plain destructure without a rest gets no injection. When the component also references `$$slots` (so the sanitize_slots const owns that name), the injected prop deconflicts by renaming — `$$slots: $$slots_` (`VariableDeclaration.js:56-73`; always the `_` suffix; `$$events` never renames; a user `$$slots_`/`$$events` reference or declaration is oracle-rejected input, so no second-order collision exists).

- **Refused**: `$props() binding pattern (not an identifier or object pattern — the oracle rejects it)`
- **Refused**: `$props() used more than once` — the oracle's `props_duplicate`, raised from its analyze-phase `CallExpression` visitor *before* the placement check (`2-analyze/visitors/CallExpression.js:68-73`), so a duplicate wins over `props_invalid_placement` when both apply. The flag is per-script, matching the oracle's own `has_props_rune`; `$props()` and `$props.id()` are tracked separately, so one of each is not a duplicate.
- **Refused**: `comments in a script with a rest-element $props() (injected $$slots/$$events)`
- **Refused**: `comments in a script with a non-destructured $props() (injected $$slots/$$events)`

---

## Runes

Sanctioned rewrites (all Supported, at parity):

| Source | Emitted |
| --- | --- |
| `$props()` | `$$props` (plus the rest injection above) |
| `$state(v)` / `$state.raw(v)` | `v` (`void 0` when argument-less) |
| `$derived(e)` | `$.derived(() => e)` — but the oracle's `b.thunk` runs `unthunk` (`utils/builders.js`), which **collapses** the arrow when its body is a call whose callee is a bare identifier and whose arguments match the (empty) parameter list one-for-one. At a thunk's arity that means an argument-less, non-optional call on an identifier: `$derived(get_library())` emits `$.derived(get_library)`. `$derived(f(a))` and `$derived(o.m())` keep the arrow. |
| `$derived.by(f)` | `$.derived(f)` |
| statement-position `$effect(…)` / `$effect.pre(…)` | dropped (and forces the context wrapper) |
| `$state.snapshot(x)` | declarator init → `x`; template position → `$.snapshot(x)` (see below) |
| `$props.id()` | hoisted `const <id> = $.props_id($$renderer)` (see below) |

A never-updated `$state`/plain binding is statically known and its template reads **fold** into the emitted text (the oracle's evaluator behavior, ported in `analyze.rs`); a template read of a non-foldable `$derived` binding — bare (`{d}`) or nested at any depth (`{d + 1}`, `{obj[d]}`, `{f(d)}`, `{d.x}`, `{!d}`) — becomes a call (`d()`), via the template value-walk (`fragment.rs::rewrite_template_value`, which rebuilds only the spine down to each derived read). The fold gate runs on the un-rewritten expression, so a foldable nested read (`{d + 1}` where `d`'s inputs are all static) still folds to text rather than emitting `d()`. A `$derived` read in a **script position** — a function body, a top-level or `$props()`-destructure-default initializer, a `$.derived(() => …)` thunk — likewise becomes `d()`, via the script rewrite (`store_rewrite.rs`, over the final synthetic body), but **never folds** (only template text folds). Writing a derived is refused where the oracle lowers it (a bare `d = v` / `d++`, or a destructuring leaf `[d] = …` / `({ d } = …)` — the oracle emits `d(v)` / an `$.to_array` IIFE, unimplemented); a **member/index** write is a read of the derived and compiles (`d.x = v` → `d().x = v`).

**`$$slots` — Supported.** A `$$slots` reference (the oracle's `uses_slots`, detected in the `needs_context` walk) injects `const $$slots = $.sanitize_slots($$props)` as the component function's first statement — before any wrapper — and forces the `$$props` parameter (`transform-server.js:300`). It reads through the rune guard's `$`-prefix refusal by a carve-out — one scoped to a **reference**, since a `$$slots` *binding* is oracle-rejected (see the binding rule below); the `$props()` rest injection deconflicts by renaming its destructured prop to `$$slots_` (see the rest-injection section). Component-wide reassignment collection also rides that walk, so a binding mutated inside a dropped event handler is still marked updated (and not folded) — and a name *declared* inside any function-like subtree (a handler param or local) marks the same-named component binding `Opaque`, whose reads refuse (`static evaluation not portable: binding {name} is not statically modeled`): the mutation target may resolve to the shadowing local, so neither folding nor escaping is provable — the script side's exact shadow envelope.

- **Refused**: `comments in a script with a $$slots reference (injected sanitize_slots)` — the injected first statement would sweep the carried-comment windows.

**`$`-prefixed bindings — Refused.** The oracle's `dollar_prefix_invalid` (`phases/2-analyze/visitors/shared/utils.js:278`, literally `node.name.startsWith('$')` on a `Binding`) is a Svelte reserved-prefix rule on a **binding**, not on a reference — which is exactly what makes the `$$slots` reference carve-out above sound, and what the carve-out must not swallow. The oracle reaches `validate_identifier_name` from **four** sites, and only two pass `function_depth` — so "oracle-rejected" is not one answer across the guarded positions. Checked against every call path, and probe-verified against the pinned compiler:

| call path | passes `function_depth`? | verdict |
| --- | --- | --- |
| `VariableDeclarator.js:25`, `FunctionDeclaration.js:12`, `ClassDeclaration.js:12` | **no** — `!function_depth` short-circuits, so the gate never applies | a declarator's binding leaves (any declaration kind, any depth, destructured included), a function-declaration id, a class-declaration id: **always rejected** |
| `scope.js:695` (`Scope::declare`) | **yes** — `function_depth <= 1` applies | a function-expression id and a catch-clause parameter: rejected **only at the top level**. An import specifier's local also declares here but is **always rejected**, by a different mechanism — `scope.js:680` re-delegates `declaration_kind === 'import'` to the parent scope, so an import binding always lands at depth 0 |

The oracle **accepts** a `$`-prefixed name at a function / arrow / snippet parameter (`declaration_kind` `param`/`rest_param` is exempt), at a template binding — `{@const}`, `{#each … as}`, an `{#each}` index, an `{#await}` `then`/`catch` value — and, inside any function body, at a function-expression id or a catch-clause parameter.

- **Refused**: `$-prefixed binding {name}` — every rejected position above.
- **Deliberate over-refusal**: a **function-expression id** and a **catch-clause parameter** are refused at *every* depth, though the oracle accepts them inside a function body (probe-verified both directions: `const g = function $$slots() {}` and `catch ($$slots)` reject at top level and accept inside `function f() { … }`; `catch ($x)` with `x` imported likewise). An over-refusal is never a refusal-contract bug, and narrowing is not portable: `WalkCtx::fn_depth` counts function *nodes*, while the oracle's non-porous increment happens at a function's **`BlockStatement`** (`scope.js:1174-1188` — a `FunctionExpression`/`ArrowFunctionExpression` scope is itself porous). So an expression-bodied arrow increments tsv's depth and not the oracle's, and a `fn_depth == 0` gate would compile the oracle-**rejected** `const h = () => function $$slots() {}` — an OVER-ACCEPTANCE, strictly worse. Buying those shapes back needs a second, oracle-shaped depth counter, for shapes no real component writes.
- **Deliberate over-refusal**: a class-**expression** id, though the oracle accepts it: the oracle's reference analysis is name-based and counts the id as a read, so `class $$slots {}` injects `$.sanitize_slots` (a mismatch), and `class $Foo {}` drives its store rewrite to emit `class $.store_get(…) {}` — invalid JS. Declining a shape no real component writes beats reproducing either. The **escaped** spelling slips through and compiles — see [conformance_svelte_compiler.md](conformance_svelte_compiler.md#-prefixed-class-expression-id-compiles-to-invalid-js).
- **Open**: an **escaped** binding name (`let $x = 1`) decodes to a name the oracle rejects, and every guarded position skips it — so **all six** are over-acceptances, probe-verified: a declarator leaf, a function-declaration id, a class-declaration id, a function-expression id, an import specifier's local, and a catch-clause parameter. Two independent mechanisms, not one: `pattern_binding_names` skips an escaped leaf (the declarator and catch-param positions), while `refuse_dollar_binding_name` reaches the other four through `dollar_identifier_name` → `identifier_name`, which returns `None` whenever `escaped_name` is set (`rune_guard.rs`). Closing the rule for escaped names means fixing both. This is an instance of the crate's standing escaped-identifier residual — its own item, not a gap specific to this rule.

**`$bindable` — Supported.** A `$bindable(fallback?)` default at a **top-level `$props()` property with a plain-identifier key and destructure value** compiles: the default is rewritten to its fallback (`void 0` when argument-less — `let { value = $bindable(42) }` → `let { value = 42 } = $$props`), the prop forces the `$$renderer.component(…)` wrapper (the oracle's `CallExpression.js:55` `needs_context`), and the component body's last statement becomes `$.bind_props($$props, { key: local, … })` — the bindable props in source order, shorthand `{ value }` when key equals local and `key: local` when renamed (`3-transform/server/visitors/CallExpression.js`, `transform-server.js`). Composes with the rest injection and with an already-firing wrapper trigger.

Every **other** `$bindable` position refuses (the oracle rejects each — `bindable_invalid_location` / `rune_invalid_arguments_length` / `rune_missing_parentheses`), left for the rune guard by surviving the rewrite unchanged: a `$bindable()` outside `$props()`, a nested default (`{ a = { b: $bindable() } }`, an array-pattern default, a nested destructure), a non-destructured `let props = $props()`, `$bindable(a, b)` (wrong arity), and a bare `$bindable` reference. Safe over-refusals (the oracle compiles, tsv declines):

- a **non-identifier key** (a string/numeric/computed key — `{ 'data-x': x = $bindable() }`) refuses via `rune {name}` (the property falls through the rewrite unchanged);
- **`$bindable` alongside carried script comments** refuses via `comments in a script with a $bindable() prop default` (`CommentsWithBindable`) — the rewrite mints appendix spans a carried-comment window would sweep.

**`$inspect` — Supported.** A **top-level statement-position** `$inspect(args)` — or `$inspect(args).with(cb)` (exactly one trailing `.with`, one callback argument) — is **dropped** in non-dev SSR: the oracle emits an empty statement that canonicalizes away, so nothing is printed. The **bare** form forces **no** wrapper. But `$inspect(a).with(cb)` and a **prop-rooted argument** (`$inspect(props.foo)`) *do* get the `$$renderer.component(…)` wrapper — not by anything `$inspect`-specific, but through the **generic `needs_context` rule** (a member/call rooted in a non-identifier, or in a prop/import binding — `needs_context.rs`): `analyze_component` walks the **raw** instance body — `$inspect` statements included — *before* the drop, so the `.with(...)` outer call (its callee's object is the `$inspect(...)` call, a non-identifier root) and the prop-rooted argument each trigger the wrapper exactly as they would in any other position. The arguments and the `.with` callback stay **rune-guarded** (a stray rune, a top-level `await`, or a `$derived` read inside refuses — so `$inspect($state(x))`, which the oracle also rejects, refuses); a comment inside the dropped region refuses (`CommentInRewrittenRuneRegion`), like `$effect`.

Refused (safe — the oracle errors or mis-compiles into invalid JS, or the position is outside the first-cut scope): a **value / template position** (`const i = $inspect(x)`, `{$inspect(x)}` — the oracle mis-compiles), a **no-argument** `$inspect()` and a **bare `$inspect` reference** (oracle errors), a **wrong-arity** `.with()` / `.with(f, x)` and a **second** `.with` (`rune_invalid_arguments_length` / mis-compile), `$inspect.trace(…)` (a **distinct** rune, not the `.with` form), and `$inspect` in a **nested function / block / `<script module>`** (the first-cut scope is a direct top-level instance `ExpressionStatement`; the oracle drops some of these, so they are safe over-refusals). All route through the `rune_guard.rs` exhaustive walk as `rune {name}`.

**`$state.snapshot` — Supported (position-dependent).** A **direct declarator init** `const s = $state.snapshot(x)` unwraps to `const s = x` — the oracle's `VariableDeclaration.js` unwraps any declarator-init rune to its argument, exactly as `$state` does. Every **non-declarator template value position** — bare `{$state.snapshot(x)}`, or nested (`{f($state.snapshot(x))}`, `{2 in $state.snapshot(x)}`, `<div {...$state.snapshot(x)}>`) — becomes `$.snapshot(<processed x>)`, a runtime call (`CallExpression.js:52`), via the **template-value substitution walk** (`fragment.rs::rewrite_template_value`): it rebuilds only the spine down to each `$state.snapshot(…)` node and processes the argument as a value in turn, so a `$derived` argument — bare or nested (`$state.snapshot(d + 1)` → `$.snapshot(d() + 1)`) — becomes `d()` and a nested snapshot becomes `$.snapshot(...)` (the snapshot walk and the derived-read walk share one node set). Refused (safe): a **script non-declarator position** (`return $state.snapshot(x)`, an assignment — deferred; the guard refuses `$state`), a **destructured declarator** (`const {a} = $state.snapshot(x)` — the oracle lowers a temp-destructure, `destructuring a $state.snapshot declarator`), a **wrong arity** (`rune_invalid_arguments_length`), and an **optional-chained** init (`$state.snapshot?.(x)` / `$state?.snapshot(x)` — see below).

**`$props.id()` — Supported.** Valid **only** as a top-level instance-script variable declarator init with a plain-identifier target and zero arguments (`props_id_invalid_placement` restricts it): the declarator is **skipped** and `const <id> = $.props_id($$renderer)` is **hoisted** to the component body's first statement (inside any `$$renderer.component` wrapper, before the `$$slots` sanitize decl — `transform-server.js:255`, placed for hydration); it forces **no** wrapper (it references `$$renderer`, never `$$props`). Every other position refuses (safe — the oracle errors): a template / attribute position, a destructured target, a nonzero-argument call (`rune_invalid_arguments`), a **duplicate** (`DuplicatePropsId` / `props_duplicate`), a nested-scope or `<script module>` occurrence, and an optional-chained `$props.id?.()`. A carried script comment alongside refuses (`comments in a script with a $props.id() declarator`, `CommentsWithPropsId`).

**Class-field `$state` — Supported.** A **top-level class declaration** whose fields include `$state(v)` / `$state.raw(v)` compiles: each such field unwraps to its argument exactly like a top-level `$state` declarator (`count = $state(0)` → `count = 0`), and a no-argument `field = $state()` becomes a **bare field** `field;` — the value is **dropped, not `void 0`** (the divergence from the argument-less top-level declarator, which mints `void 0`). A **private** (`#count = $state(0)`) and a **quoted-key** (`'aria-pressed' = $state(false)`) field unwrap the same way; non-rune members (plain fields, methods, `static` non-rune fields, getters/setters, static blocks) clone through unchanged, **member order preserved** (`script_rewrite.rs::rewrite_class_state_fields`). The unwrap set exactly equals the guard-exempt set — every member that is not a direct, non-static, non-computed `$state`/`$state.raw` field flows through the normal refusing guard walk (`rune_guard.rs::walk_class_member_guarded`) — so no member is exempted without a matching unwrap (which would emit an undefined `$state` reference).

A field whose **whole** argument is a **lone reactive-binding identifier** — a store read `field = $state($count)` or a `$derived` binding `field = $state(d)` (`$state.raw` too) — **refuses** (`class-field $state with a lone store/$derived argument (the oracle keeps it bare)`). The oracle keeps such a lone read **bare** in the unwrapped field (`x = $count` / `x = d`), NOT feeding it through the subscription / derived-call pass — but tsv's store rewrite descends into class bodies unconditionally and would rewrite the kept argument to `$.store_get(…)` / `d()`, a corpus-invisible MISMATCH. A narrow, safe over-refusal keyed on exactly "would the store rewrite touch this lone identifier?" (`store_read_base` + `store_names` / `derived_names`, escaped identifiers skipped like the store rewrite): a **compound** argument (`$state($count + 1)` → `$.store_get(…) + 1`, `$state(d + 1)` → `d() + 1`) and a **plain-variable** argument (`$state(n)` where `n` is a plain `$state`/prop) still compile at parity.

Refused (this slice): a **`$derived` / `$derived.by` class field** (the oracle emits a `#f = $.derived(…)` + get/set accessor pair — a separate slice; refuses `rune $derived`), a **`static`** rune field and a **computed-key** rune field (both `state_invalid_placement` — the oracle rejects them; refuse `rune $state`), a **constructor first-assignment** `this.x = $state(0)` (the oracle accepts → `this.x = 0`, deferred — the method body takes the refusing guard walk), and a `$state` field in a **nested class** or a **class expression** (only a top-level class declaration is reached; refuses `rune $state`). Every other rune in a class-field position refuses too.

An **optional-chained rune init** — `$state?.(x)`, `$state.snapshot?.(x)`, `$state?.snapshot(x)`, `$props.id?.()`, `$derived?.(e)`, … — is a `ChainExpression` the oracle's `get_rune` does not see through, so its declarator-unwrap never applies (the placement-restricted runes then error; `$state.snapshot`, valid anywhere, has its `CallExpression` visitor emit `$.snapshot`). `classify_rune_init` refuses to classify any optional-chained init (`call.optional` or an optional callee member), so tsv declines it — a safe over-refusal for `$state.snapshot` (where the oracle would emit `$.snapshot`) and a matching rejection for the rest. The **template** snapshot path stays optional-agnostic and emits `$.snapshot(x)` at parity.

Everything else `$`-shaped refuses (the `rune_guard.rs` exhaustive walk):

- **Refused**: `rune {name}` — any non-sanctioned rune call (`$effect.tracking`, `$host`, member-form misuse, a rune call in any non-sanctioned position, or a `$bindable` / `$inspect` / `$state.snapshot` / `$props.id` outside its sanctioned position — see the rune sections above)
- **Refused**: `$-prefixed identifier {name}` — a bare rune reference (oracle-rejected input) or a `$`-prefixed identifier read whose base is **not** a component binding (a valid `$name` store access is exempted — see Stores below)
- **Refused**: `read of derived binding {name}` — an unsupported read position. A `$derived` read (bare or nested) rewrites to `d()` in both template value positions and script positions (above), so this refuses only the positions no rewrite reaches: a **template pattern default** (`{#each xs as {v = d}}` — the oracle emits bare `d`, a deferred safe over-refusal; `{#await p then {x = d}}` — the oracle emits `d()`, so refusing is mandatory), a read under an **unsupported wrapper** (an object literal, an arrow, a tagged template), an **escaped-identifier** derived read (`{d}` — classification not ported; refused rather than emit bare `d`), and a `$derived` name **shadowed** by a nested-scope local (`DerivedReadShadowed`, a safe over-refusal for the name-based rewrite)
- **Refused**: `destructuring a $state declarator` / `destructuring a $state.snapshot declarator` / `destructuring a $derived declarator` / `destructuring a $derived.by declarator`
- **Refused**: `binding pattern shape ({kind})` — a `$props()`-family binding whose pattern the analyzer doesn't classify
- **Refused**: `top-level await (async component output not implemented)`
- **Refused**: `rune {name} whose base is also an instance binding` — a rune keyword whose `$`-stripped stem is *also* a binding **in scope at the instance script** (`import { state } from './store'` beside a `$state` reference). The oracle's `analyze_component` reclassifies such a reference as a **store subscription** on that binding, not as the rune, and deletes it from `module.scope.references` before it infers runes mode — so the collision can flip the whole component out of runes mode. tsv models neither, so it refuses rather than compile the rune. The scope tested is the oracle's `instance.scope.get`, which walks **up** the chain into the **module** scope (so a `<script module>` binding collides too) it never walks **down**, so a function parameter, a block-scoped `let`, or a name bound in a nested function body does *not* collide and keeps compiling. Two nested forms still reach script scope, and they differ: a function-scoped **`var`** in any block, for-head, switch, or try/catch — which the oracle re-declares on the parent **without its initializer** (`phases/scope.js:673-681`), so no rune init can exempt it — and a declaration in a class **static block**, which `phases/scope.js` gives no scope at all (there is no `StaticBlock` visitor), so it declares at script scope with its initializer **intact**. The two are handled asymmetrically, deliberately: the `var` hoist is modelled **exactly** (one exhaustive `Statement` enumeration, `each_script_declaration`), while the static block is **fenced lexically** — a component containing any `static { … }` refuses on its first rune reference, whatever that block declares. Reaching every class body a script can hold means enumerating every expression position of every statement (a for-head, a `super_class`, a property initializer — which is **not** a function scope, `phases/scope.js` having no `PropertyDefinition` visitor either — a computed key, a function parameter default), and a hand-enumerated version of that surface shipped silent MISMATCHes twice. A static block is `static`, then trivia, then `{`, and its token always sits inside a statement's span — so the fence misses one only by mis-classifying the trivia, and its completeness is exactly the completeness of its **whitespace class**. It matches ECMAScript `WhiteSpace`/`LineTerminator` (`text_class::is_js_whitespace`), not Rust's `char::is_whitespace`: the two differ at `U+FEFF` (ECMAScript whitespace, but with no Unicode `White_Space` property), and `static\u{FEFF}{ … }` was invisible to the fence, compiling the rune where the oracle emits a store read. It over-reports harmlessly; its measured parity cost is **zero** — no `.svelte` file in the ~4900-file compile corpus contains a static block at all. The oracle's **exemption** — a binding whose `initial` *is* a rune call (`let state = $state(0)`, `const props = $props()`) — is modelled, so the common shapes keep compiling; so are the corners of its clause (`let state = $props()` **is** reclassified; `$derived` beside `import { derived } from 'svelte/store'` is **not**; a rune-initialized `var` hoisted through a porous scope **is**). Corpus-invisible — found against Svelte's own `validator` / `compiler-errors` suites.

A `$`-prefixed *member name* (`a.$foo`) is not a rune reference, and the rune guard leaves it alone. It is not unconditionally compilable, though: the collision pre-pass above uses a whole-document source scan, so `obj.$state` (or `{ $state: 1 }`, or `$state` in a comment or in template text) counts as a `$state` reference and refuses whenever `state` is *also* a binding in scope — a deliberate over-refusal, never a wrong compile.

### Stores (`$name` auto-subscription) — Supported

A `$name` reference whose `$`-stripped base is a top-level component binding (an import OR a local `let`/`const`, and not a rune keyword — `store_read_base`) is a store auto-subscription. Reads and writes are lowered to the oracle's SSR runtime calls:

| Shape | Emitted |
| --- | --- |
| **read** `$count` (template OR script, ANY value position — a declarator init, a function body, a binary/conditional, a **callee** `$fn()` / `$obj.m()` / `new $C()`, at any depth) | `$.store_get(($$store_subs ??= {}), '$count', count)` (`Identifier.js` → `serialize_get_binding`); a `$derived` base reads `count()` |
| **assignment** `$count = v` | `$.store_set(count, <v rewritten>)` (`AssignmentExpression.js`) |
| **compound** `$count += v` | `$.store_set(count, $.store_get(…) + <v>)` (reconstructing the oracle's `build_assignment_value`) |
| **postfix update** `$count++` / `$count--` | `$.update_store(($$store_subs ??= {}), '$count', count[, -1])` (`UpdateExpression.js`) |
| **prefix update** `++$count` / `--$count` | `$.update_store_pre(($$store_subs ??= {}), '$count', count[, -1])` |

The script rewrite lives in `store_rewrite.rs` (a tree→tree pass over the final synthetic body, so a read inside a `$.derived(() => …)` thunk is reached); the template read stays in `fragment.rs::rewrite_template_value`. Either presence — read or write, **emitted or dropped** (an event handler, `{:catch}`) — makes `needs_context` set `uses_stores`, which injects `var $$store_subs;` (component-body top) and `if ($$store_subs) $.unsubscribe_stores($$store_subs);` (last statement); a store access does **not** force the `$$renderer.component(…)` wrapper. Refused (safe over-refusals — the oracle compiles, tsv declines this slice):

- **member write** `$obj.foo = 5` / `$obj.foo++` → the oracle emits `$.store_mutate`; refuse (`store member write`)
- **destructuring write** `[$count] = arr` / `({ x: $count } = obj)` → the oracle builds an IIFE; refuse (`store destructuring write`)
- **scoped subscription** `$count` whose base is bound in a nested scope → the oracle's `store_invalid_scoped_subscription` error; refuse via a name-based shadow check (`store_shadowed` = `nested_declared` ∪ `component.fn_declared`), which correctly refuses the true shadow and over-refuses a harmless sibling-scope collision (both safe). A store read in a callee/new position (`$fn()`, `new $C()`) is exempted from the guard's rune refusal exactly like a bare read (`rune_guard.rs::store_read_exemption`), and a shadowed callee refuses the same way
- **template-position write** `{($count = 5)}` → refused via `DollarPrefixedIdentifier` — the template value guard trips on the `$count` read before the `updated`-nonempty check, since a store write is not a template value-walk rewrite target (only script + dropped-handler writes are in scope). `MutationInTemplateExpr` fires only for a **non-store** template mutation `{(x = 5)}`
- **rune-keyword base** `let state = writable(0); {$state}` → `$state`'s base `state` is a `RUNE_BASES` keyword, so `store_read_base` returns `None` and it is never recognized as a store — a deliberate conservative over-refusal shared with the template path (a `$name` whose base collides with a rune keyword is refused as a bare `$`-prefixed identifier), not introduced by this slice

---

## Script Statements

Instance-script statements are borrowed verbatim (with the rune rewrites applied) into the component function.

- **Supported**: declarations, functions, classes, expression statements, control flow — any statement shape the guard walk covers, with comments carried through losslessly (host-absolute spans).
- **Supported**: `lang="js"` and `lang=""` (compile exactly like no `lang` attribute).
- **Refused**: `instance-script export (component exports / $.bind_props not implemented)` — every export form: the oracle compiles `export const`/`function`/`{ a }` via `$.bind_props`, rejects `export default`/`export let` (runes mode), and drops `export * from`; a verbatim passthrough would nest an `export` inside the component function. A **type-only** export (`export type { X }`, `export interface X {}`, `export declare const x`) erases away before this refusal and compiles.
- **Refused**: `` legacy reactive statement `$:` (invalid in runes mode) `` — a **top-level** `$`-labeled statement (the oracle rejects it in runes mode; cloning it through would emit a dead label with no reactivity). A `$` label inside a function, and plain labels anywhere, are ordinary JS the oracle clones through — supported. An escaped top-level label name refuses conservatively (can't be classified from its raw span).
- **Refused**: `import from svelte/internal (forbidden)` — any import whose source starts with `svelte/internal` (the oracle's runes-mode rule; private runtime code)
- **Refused**: `runes-invalid import of {name} from svelte` — a named `beforeUpdate`/`afterUpdate` import from `svelte` (the oracle rejects them in runes mode); an escaped imported name from `svelte` refuses conservatively. A string-literal imported name is skipped exactly as the oracle skips it (its check matches identifier names only).
- **Refused**: `lang="{lang}" script` — only `ts`/`js` are supported; any `lang` other than `ts`/`js`/empty (on the instance **or** module script). The oracle's TypeScript flag tests `lang === 'ts'` **exactly** (case-sensitive), so `lang="typescript"` / `lang="TS"` are plain JS to it; rather than compile them as JS on a guess, tsv refuses.
- **Refused**: `generics attribute on <script> (implies TypeScript)` — an open type-parameter *binding*, not annotation erasure (a separate slice).
- **Refused**: `generated name {name} collides with a user binding` — a user binding named `each_array`/`$$index`-family

### Module Scripts (`<script module>` / `<script context="module">`) — Supported (plain)

A **plain** (rune-free) module script compiles. Its type-free body — imports, `const`/`let`/`var`/`function`/`class` declarations, non-default exports (`export const`/`function`/`class`/`{ x }`/`{ x } from`/`* from`), and plain statements — emits **verbatim** (post-erase) as its own comment-free module-scope program, placed **between the hoisted snippets and the component function** (the oracle's placement: the whole module block follows the hoisted snippets, NOT merged into the instance import group; module imports stay inline within it). Module bindings join the shared table, so a module `const K = 5` folds `{K}`, a module store feeds a template `{$c}` subscription, a module import member/call fires the `$$renderer.component(…)` wrapper, and a module `let` reassigned anywhere stays dynamic.

- **Supported**: TypeScript erasure under the document `lang="ts"` flag — which a `lang="ts"` **module** can set (the flag is the first lang-bearing script in source order, module or instance).
- **Refused**: `default export in <script module> (the oracle rejects it)` — the oracle's `module_illegal_default_export`.
- **Refused**: any module-scope **rune** (`$state`/`$derived`/…) via the rune guard — v1 defers the oracle's module rune rewrites (the corpus is module-rune-free, so this is a lossless over-refusal); v2 reclaims.
- **Refused**: a module-scope `$name` **store read** (the oracle's `store_invalid_subscription`) and a module top-level `await`, both via the rune guard.
- **Refused**: `binding {name} declared in both the module and instance scripts` — the oracle resolves a template `{name}` read to the instance (inner-scope) binding, but the name-based table would overwrite it with the module binding and fold the module value; the port can't disambiguate which scope a reference resolves to, so refuse rather than MISMATCH.
- **Dropped (parity)**: module-script **comments** — the oracle drops every module comment, so the module body emits comment-free.

### TypeScript — Supported

`<script lang="ts">` compiles: type erasure runs as a pre-pass over the instance script's `Program` (`erase.rs`), matching the oracle's phase-1 `remove_typescript_nodes` (`1-parse/remove_typescript_nodes.js`), which runs before its analysis phases (`index.js:41-53`). The Svelte AST is never rebuilt — a **type-free** statement list flows into every analysis and into codegen.

TypeScript in the **template** is erased too, per-expression **at the borrow point**: every TypeScript-bearing markup position is a `tsv_ts` expression (or one `Option<TSTypeParameterDeclaration>`) reached through a small set of borrows — `{expr}` / `{@html}` tags, attribute values (single, mixed, component prop, component spread), block tests, `{@render}` calls, and the four pattern positions (`{#each}` context, `{:then}` value, `{@const}` binding, `{#snippet}` parameters). The erased node is what *every* consumer of the borrow sees: not only the emitted argument but the **static-fold gate beside it** (`{x as T}` would otherwise evaluate to UNKNOWN where the oracle folds `x` — a silent under-fold, a parity divergence no refusal catches) and the shape predicates that read a node's variant (`class={'a' as T}` is a string literal to the oracle, not a `$.clsx` candidate; `<Foo n={n as T} />` is the `{ n }` shorthand).

The oracle's flag is **document-wide**: its parser regexes the raw source for the *first* `<script>` carrying a `lang` attribute and tests `=== 'ts'` exactly. That one flag selects the TypeScript grammar for every `<script>` **and** every template mustache, block pattern, and snippet `<T>` clause — so tsv makes one document-level decision too.

| Construct | Behavior |
| --- | --- |
| `: T` annotations (bindings, params, properties, return types) | erased |
| `interface` / `type` aliases | dropped |
| `import type { X }` / `export type { X }` / `export interface X {}` | dropped |
| inline `import { type X, Y }` | the type-only specifiers are filtered out; a list that filters to **empty** drops the whole statement (the oracle's `if (specifiers.length === 0) return b.empty` — not `import {}`, not a bare side-effect import) |
| `x as T` / `x satisfies T` / `x!` / `<T>x` / `f<T>` | unwrapped to the inner expression (`as const` included) |
| `/** @type {T} */ (x)` (a JSDoc cast — valid JS, not TypeScript) | unwrapped: the oracle parses without `preserveParens` and has no such node, so it prints the JSDoc as a detached leading comment, drops the parens, and folds the value |
| `constructor(override x: number)` | unwrapped — the oracle rejects a parameter property **only** for `readonly`/an accessibility modifier in a constructor (those synthesize `this.x = x`) |
| `f<T>(x)` / `new C<T>()` / tagged-template type args | type arguments dropped |
| `<T>` type parameters (function / arrow / class / method) | dropped |
| `declare` variable / function / class / class field | dropped |
| overload signatures (`TSDeclareFunction`) | dropped |
| `abstract` class + `abstract` **method** (no body) | dropped |
| `readonly` / `public` / `private` / `protected` / `override` / `?` / `!` modifiers | dropped |
| `implements` clause, `extends Base<T>` type arguments | dropped |
| leading `this: T` parameter (function declarations/expressions only, never arrows — the oracle's `remove_this_param`) | dropped |
| **type-only** `namespace`/`module` | dropped (the oracle's all-type→drop fork) |
| template `{x as T}` / `{x!}` / `{x satisfies T}` / `{f<T>(x)}`, in a tag, an attribute value, a component prop or spread, a block test, a `{@render}` argument | erased at the borrow point (then folded/guarded like any expression) |
| typed block patterns — `{#each xs as x: T}`, `{#await p then v: T}`, `{@const a: T = v}` — identifier **and** destructuring forms | erased at the borrow point |
| `{:catch e: T}` | **not erased — never reaches output.** The oracle drops the whole `{:catch}` branch from SSR, so its binding is emitted nowhere. (Its TypeScript is still *seen*: without `lang="ts"` the dropped-region sweep refuses it.) |
| typed and **generic** `{#snippet s<T>(x: T)}` | erased: the oracle emits `function s($$renderer, x)`, the `<T>` simply gone — a snippet's type parameters are type-level only, so *not reading them* is the erasure |

Parens are not a hazard: `tsv_ts` parses with `preserve_parens: false` and re-derives them from precedence, exactly as the oracle's printer does — `(x as T).y` erases to `x.y`, and `(a + b as T) * c` keeps the parens it needs.

**The self-check.** `compile`'s output-reparse validation **cannot** catch a missed erase: tsv's parser is TypeScript-permissive (see the root `CLAUDE.md` §Strict Mode Only), so a surviving annotation still parses, flows through the pipeline, and prints verbatim. The eraser is therefore re-run over the *finished* program: by its `None`-means-unchanged contract, reporting no change **proves** no TypeScript-only node survived. Both halves of the erasure — the script `Program` and each template expression — run before it, so **any** survivor is a compiler bug (`CompileError::TypeErasureLeak`, surfaced loudly, never emitted): a missed erase case, or a borrow point that never called the eraser. It is why a missed borrow point cannot silently ship TypeScript.

### TypeScript — Refused

- **Refused**: `TypeScript syntax without lang="ts" (the oracle parse-errors)` — tsv's parser accepts TypeScript everywhere; the oracle's grammar is gated on the document-wide flag, so without it TypeScript **anywhere** in the document is a parse error. Compiling it would be an over-acceptance. The script is covered by the erase pre-pass and the template by a sweep (`refuse_template_typescript`) that runs *only* when the flag is absent — it exists for the positions the erase self-check can never see, because SSR **drops** them: the `{#each}` key, the `{#key}` expression, an event handler, and the whole `{:catch}` branch.
- **Refused**: `comment inside an erased TypeScript region` — the oracle's surviving-comment placement is an *emergent* artifact of its printer's flush points reading pre-erasure spans (RHS-leading for a declarator, statement-trailing for an `as`, argument-leading for a call type argument, hoisted-to-the-next-statement for a deleted `interface`), not a rule with a portable shape. The refusal **window** is wider than the erased span on both sides: **forward** to the start of the next surviving token (so `let x: Foo /* c */ = v` — which the oracle re-anchors onto the initializer — is caught), and **backward** to the end of the previous surviving token for a region *detached* from it (a `return_type` after `)`, an `implements` clause, a `<T>` list — the printer never queries the erased node's byte range, but the enclosing node's gap window still spans it, so the comment would print anyway). A whole-statement drop deliberately does **not** reach backward: a *leading* JSDoc above an erased `interface` survives and lands on the next statement, exactly where the oracle puts it.

**Refuse-don't-erase.** Constructs with runtime semantics an erasure would silently delete, plus the ones the oracle itself mis-compiles. Zero occurrences across the real-world corpus.

- **Refused**: `TS enum (the oracle rejects it)` — lowers to an object plus a reverse mapping. The oracle's visitor has **no `declare` carve-out**, so `declare enum` is rejected too.
- **Refused**: `TS namespace/module with a value member (the oracle rejects it)` — lowers to an IIFE (the oracle's any-value→reject fork).
- **Refused**: `dotted TS namespace A.B (the oracle crashes on it)` — the strip visitor assumes a block body and calls `node.body.body.map(…)` on the nested module declaration; it throws, at any body content.
- **Refused**: `TS parameter property with readonly/accessibility (the oracle rejects it)` — real TypeScript synthesizes `this.x = x`. Exactly the oracle's rule: a lone `override`, or a modifier outside a constructor, is unwrapped and compiles.
- **Refused**: `decorator (the oracle rejects it)` — a `typescript_invalid_feature` hard error in the oracle, and a plain-JS parse error without `lang="ts"`.
- **Refused**: `accessor class field (the oracle rejects it)` — likewise a hard error.

The next four are cases where the oracle's strip pass has **no visitor case**, so the construct survives into its output: tsv refuses rather than reproduce a broken module (the same stance as `import = require`, and the refusal contract covers it — no divergence-catalog entry).

- **Refused**: `abstract class property (the oracle emits invalid JS)` — the oracle prints `abstract x;`. (An `abstract` *method* is dropped — the split is by node kind, never by body-presence.)
- **Refused**: `bodiless class method (overload signature — the oracle rejects it)` — the signature survives and collides with the implementation (`duplicate_class_field`).
- **Refused**: `index signature in a class body (the oracle crashes on it)` — a pure type construct, but the oracle's transform throws.
- **Refused**: `import x = require(…) (the oracle emits invalid JS)` / `export = … (the oracle emits invalid JS)` / `export as namespace … (the oracle emits invalid JS)` — all three land inside the component function verbatim.

### Comment placement classes

Instance-script comments carry through by default, regardless of what the
template emits: a script comment is a leading comment of a surviving script
statement, and every template emitter (blocks, component invocations, expression
attributes, the drop family) writes only template-region spans, which no
script-comment window can reach. Hoisted imports are no obstacle — the oracle
relocates every script comment down into the component body (leading the first
surviving statement) with the imports hoisted comment-free, and tsv reproduces
that.

- **Supported**: comments alongside template blocks (`{#if}`/`{#each}`/`{#await}`/`{#key}`/`{@const}`), a component invocation, `{#snippet}`/`{@render}` (hoisted or body-local), expression-valued attributes (`class={c}`, `style:` / `class:` / `bind:` directives, `{...spread}`), hoisted imports (a comment before/between/after imports relocates down to lead the first surviving statement, as the oracle does), and a **`$derived(e)`/`$derived.by(f)` declarator** (the synthetic `$.derived(…)` and its arrow steal the replaced init's host span — `build.rs::derived_call` — so a following script comment flows to the next statement instead of being swept into the derived slot).

A comment **past the last surviving statement** (imports hoist, `$effect`/`$inspect`
drop, so an import-only script has none) carries too: with no statement left to
lead it falls to the end of the synthetic function body, whose block span runs
`[content.start, rbrace_end)` and captures it exactly once. The oracle instead
re-attaches it into the template — trailing the final push, or nested inside the
next emitted node (an `{#if}` condition, an `$.ensure_array_like(…)` /
`$.attr(…)` argument) — a position difference the parity bar tolerates. The one
carve-out is a template that emits a **nested block**, which refuses (see below).

**Comment position is tolerated, not pinned.** A carried comment that tsv places
by its own comment philosophy where the oracle (esrap) relocates it — a comment at
an operator / conditional boundary inside an expression — still reaches parity: the
parity bar tolerates a comment-*position* difference (same code, same comment
sequence). See the crate `CLAUDE.md` §The Parity Bar.

The classes that still refuse are the ones where the comment has no surviving
anchor and the oracle re-anchors it in a way the span-window model can't reproduce,
or where a rune rewrite mints a script-region span a comment window would sweep:

- **Refused**: `comment after the last script statement in a template that emits a nested block (the oracle drops it)` — the oracle's printer walks one comment index; opening a block with no source `loc` resets it to the end, discarding every comment not yet written, while opening a block that has a `loc` re-seeks that index absolutely and so can move it backward. A loc-less block therefore annihilates the index and the next loc-bearing one recovers it — which is how the comment survives the component body (that block is assigned the instance script's `loc`, and a context-wrapped component wraps it in a fresh loc-less block, so the wrapper annihilates and the inner block seeks back). A template block gets no such recovery, so the comment vanishes from the oracle's output while tsv keeps it — a DROP, which the parity bar grades. The scan (`script_rewrite.rs::template_emits_nested_block`) asks only whether a synthetic block exists anywhere — `{#if}`/`{#each}`/`{#await}`/`{#key}`/`{#snippet}`, a special element, or a component with children — not whether one is reached before the comment would flush, so it over-refuses the common case where a loc-bearing head expression (an `{#if}` test) flushes the comment first, and likewise the special elements that emit no block at all (`<svelte:window>`, `<slot>`). The split is keyed to the pinned oracle's `reset_comment_index` behavior (esrap 2.2.12) — re-probe it if that pin moves
- **Refused**: `leading comment glued to the <script> line (no newline before it)`
- **Refused**: `multi-line block comment in script (interior-line re-indentation not carried through)` — the oracle re-indents a block comment's interior lines to the emit position; tsv carries them verbatim
- **Refused**: `comments with template markup before the script (window ordering)`
- **Refused**: `comment inside a rewritten rune region (dropped by the transform)` — includes a comment INTERIOR to a `$derived(e)`/`$derived.by(f)` argument, whose synthesized `() => …` arrow would double-print it (the whole derived init is a dropped region; a comment *around* the derived declarator still carries)
- **Refused**: `comments in a script that references a store ($$store_subs injection)` — the `var $$store_subs;` injection (and the `$.store_get`/`$.store_set` mints) are synthetic spans whose windows would sweep the carried comments; fires for a template-only `$name` read too (`CommentsWithStore`)
- **Refused**: `comments in a script with an argument-less $state()`
- **Refused**: `format-ignore directive comment in script`
- **Refused**: `template comments (only instance-script comments are carried through)`

---

## Template

### Static emission — Supported

The oracle's normalization (`3-transform/utils.js:126` `clean_nodes`, `escape_html`), probe-verified: whitespace-only boundary text drops and edge runs trim per fragment; a text edge run abutting a non-text node collapses to one space (text + `{expr}` count as one text) — **removed entirely** under the `svg` namespace (inferred per fragment, `namespace.rs`) except inside `<text>`, and under the select/table-family parents; interior whitespace is verbatim; `<pre>`/`<textarea>` preserve everything; entities decode then re-escape (`[&<]` in text, `[&"<]` in static attributes); boolean attributes emit `name=""`; `class`/`style` values collapse+trim; a string-valued `class` that collapses+trims to empty is dropped entirely (static path only — bare `class` keeps `class=""`, empty `style`/`id` stay, a *folded* mixed class keeps `class=""`); void elements close `/>`; a text-first fragment (component root or `{#each}` body — `3-transform/utils.js:295` `is_text_first`) gets a `<!---->` prefix.

### Expressions — Supported

- `{expr}` → `$.escape(expr)`; statically-known values fold as text; a derived read (bare or nested) becomes `d()`.
- `{@html expr}` → `$.html(expr)`.
- **Refused**: `{@html} with a statically-known value` (the oracle folds it)
- **Refused**: `mutation inside a template expression`
- **Refused**: `static evaluation not portable` / `static fold not portable` — each carries a `{reason}` in its `Display` message. A statically-known value whose byte-exact stringification isn't proven (the evaluator's bounded domain)

### Blocks

| Block | Status |
| --- | --- |
| `{#if}` / `{:else if}` / `{:else}` | Supported (flat chain, numbered anchors, synthesized terminal else) |
| `{#each}` (with `{:else}`, authored index, sibling numbering) | Supported |
| nested `{#each}` | **Refused**: `` nested {#each} (the nested emission path is not yet validated) `` |
| `{#await}` / `{:then}` (`{:catch}` dropped, matching the oracle's server output) | Supported |
| `{#key}` | Supported |
| `{@const}` (hoisted to branch top, enters the evaluator) | Supported |
| `{@const}` edge shapes | **Refused**: `{@const} at the component root (only valid inside a block)`, `` destructured {@const} (only `{@const name = …}`) ``, `{@const} with a non-plain binding name`, `{@const} outside a block scope` |
| shadowing | **Refused**: `block-scope binding {name} shadows a $derived binding` |
| `{@debug}` / `<!-- html comments -->` / declaration tags | **Refused**: `template node {kind}` (kinds: `{@debug} tag`, `html comment`, `declaration tag`) |

**Emission-dropped regions are still walked.** The SSR output drops four template regions — the `{#each}` key, the `{#key}` expression, an event-handler attribute, and the whole `{:catch}` branch — and a region the emitter never *visits* is a region no emission refusal can fire in. But the oracle decides several things *before* it chooses what to emit, so dropping a region cannot make an invalid component valid. Every dropped region is therefore walked anyway, for exactly what the oracle decides early:

- **TypeScript** — a *parse*-phase decision (the document-wide `lang="ts"` gate above);
- **misplaced runes** — an *analysis*-phase error (`{:catch e}{$state(1)}{/await}` is `state_invalid_placement`);
- **references** — the oracle counts them wherever they sit, so a dropped region's references still drive `needs_context` and block a `{#snippet}`'s module hoist (`attr_refs.rs`'s dropped-fragment view; a `{:catch}` the emitter discards is the reason that view exists);
- **presence-read constructs** — a fact the oracle's phase 2 keys on a node (or an attribute on one) *existing*, which dropping the region cannot suppress. These run on two axes, covered below.

What a dropped region does **not** get is the *emission* refusals: a spread or a `{@debug}` inside a `{:catch}` compiles, because the oracle drops it too — and neither does the derived-read rule, which is an emission rewrite (`d` → `d()`), not a validity rule. Refusing there would cost parity on shapes the oracle accepts.

#### The two presence-read axes

The line between the last two bullets is **"can it affect the result from here"**, not "is it fenced". A dropped construct can reach the result two ways, and the second is the one a per-construct probe cannot see:

- **Emission** — the fact rides into the generated code. A **`<slot>`** records into `analysis.slot_names`, and `slot_names.size > 0` folds into `should_inject_props`, so a `<slot>` in a `{:catch}` widens the signature to `($$renderer, $$props)` while SSR emits nothing from the branch. It **refuses** (`template node special element <slot>`, the emitted path's own bucket — the fence firing in a second position, not a new reason). Measurable one construct at a time: compile with and without it and diff.
- **Validation** — the fact feeds a whole-component check in `2-analyze/index.js` that can turn an otherwise-valid component into a compile *error*. A legacy **`on:`** sets `analysis.event_directive_node` (`visitors/OnDirective.js`); an `onclick`-style attribute on an emitted element sets `analysis.uses_event_attributes` (`visitors/Attribute.js`); together they raise `mixed_event_handler_syntaxes`. So `{:catch}<button on:click=…>` plus a sibling `<div onclick=…>` makes the oracle reject a component tsv would compile. It **refuses** (`legacy on: directive (runes-only fence)`).

⚠️ An isolated probe answers the **emission** axis only. A construct that compiles byte-identically with and without it, measured alone, may still be on the validation axis — those checks are whole-component, so they need a *second* construct elsewhere to fire. Do not read "inert in isolation" as "inert".

**Known open holes** on the validation axis — both over-acceptances (tsv compiles what the oracle rejects), neither reachable from the real-component corpus (`compile_fuzz` reaches both, which is what an adversarial generator is for):

| Dropped construct | Emitted partner | Oracle error |
| --- | --- | --- |
| `{$$slots.x}` | `{@render …}` | `slot_snippet_conflict` |
| `{#snippet s()}` | `export { s }` from a module script | `snippet_invalid_export` |

Neither `$$slots` nor `{#snippet}` is a fence — tsv intends to support both — so closing these means porting the oracle's whole-component validations, not widening the presence match. That is separate work from the dropped-region walk.

**Everything else keeps compiling** in a dropped `{:catch}`: `<svelte:component>`, `<svelte:self>` (under an `{#if}`), `<svelte:fragment>` and a `slot="…"` component child (both as a component's children), plus the unfenced `<svelte:element>` and `<svelte:boundary>`. That set is clean on both axes — verified by reading the writers, not by probing: the whole-component fields a phase-2 validation reads (`slot_names`, `uses_slots`, `uses_render_tags`, `event_directive_node`, `uses_event_attributes`, `snippets`) are written only by `SlotElement` / an `$$slots` `Identifier` / `RenderTag` / `OnDirective` / an event `Attribute` / `SnippetBlock`, and none of those constructs is one of them. Refusing them to make the fence uniform would trade correct output for nothing. `let:` is likewise on neither axis (its only check, `let_directive_invalid_placement`, is local to its parent) but refuses anyway, to keep the fenced `on:`/`let:` pair in one census bucket. Only the placement-restricted metas (`<svelte:head>`, `<svelte:window>`, `<svelte:body>`, `<svelte:document>`) are unreachable, rejected by the oracle inside any block.

`dropped_fragments_are_walked` pins the expression halves; `dropped_fragment_refuses_presence_read_nodes` pins both presence axes **and** the must-not-over-refuse set beside them.

#### `analysis.elements` — presence-read, and the census follows it

`RegularElement.js` and `SvelteElement.js` push **every** element into `analysis.elements`, which drives CSS pruning (`2-analyze/index.js` → `prune(analysis.css.ast, analysis.elements)`). An element in a `{:catch}` therefore keeps a CSS rule alive in the oracle's output.

tsv's element census **descends all three `{#await}` arms**, `{:catch}` included, even though the catch arm never reaches SSR output. Pruning is decided before emission is — `css-prune.js:1110-1111` pushes `pending`/`then`/`catch` alike — so a selector matching only catch content is KEPT and scoped, exactly as the oracle does. Excluding it previously made such a selector match nothing and over-refuse (`css selector … matches no element`).

Marking an element emission never reaches is safe: `element_scope` is a span lookup performed at emission, so an unemitted element is never queried and contributes nothing to the output. This is the same widening `<svelte:boundary>` needs for its `pending`-discarded children — but it needs no distinct census owner, because `is_block` holds `AwaitBlock` (`css-prune.js:1240-1246`), so the upward sibling walk continues through it as `Owner::Await` already models. The census leaf set is now deliberately **wider** than the emitted set in exactly these two places.

### The wider validation surface

The two rows above are the dropped-region slice of a **general** hole: tsv's compiler
implements the oracle's *emission*, not its *analysis*, so a component the oracle's
analyzer rejects can still compile. Each row below is an over-acceptance with a
standalone repro, none of them dropped-region-specific:

> **This inventory is now GATED, not just described.** Svelte's own `compiler-errors` +
> `validator` suites — 455 files, ~2/3 deliberately invalid — are a standing corpus behind
> `deno task compile:validation`, a path-keyed known-bug ratchet over the
> over-acceptances they expose (`compile_validation_known.txt` is the count — a figure
> repeated in prose only goes stale, and this one had). A *new* over-acceptance
> fails the gate; a pinned one that stops firing fails too, so closing a rule forces
> removing its lines and the list cannot rot. The prose below stays the *diagnosis*; the
> snapshot is the *measurement*. See
> [compile_validation_ratchet.md](compile_validation_ratchet.md).

| Repro | Oracle error |
| --- | --- |
| `<h1><h1>t</h1></h1>` | `node_invalid_placement` |
| `<Foo><p>c</p>{#snippet children()}<p>d</p>{/snippet}</Foo>` | `snippet_conflict` |
| `{#await Promise.resolve(1)}<i>p</i>{:catch e}<Foo class:a={true} />{/await}` | `component_invalid_directive` |

Together with `attribute_duplicate`, `declaration_duplicate`, `snippet_invalid_export`,
`slot_snippet_conflict`, `svelte_meta_invalid_placement`, and `svelte_meta_duplicate`,
that is **nine** distinct oracle rules tsv does not enforce — so the shape of this
work is porting `2-analyze`'s whole-component checks, not patching rules one at a time.
(`dollar_prefix_invalid` was enforced first, and the three-rule `validate_assignment`
family — `constant_assignment`, `each_item_invalid_assignment`,
`snippet_parameter_assignment` — after it; see below and the `$`-prefixed bindings rule
above. Both came out ahead of the rest because each is a **local** rule on a single node
whose inputs a name-based port can supply, unlike the whole-component checks that remain.)

⚠️ **These are Svelte *analysis-phase* rules, not deferred JS early errors** — do not
file them under the parser's [deliberate early-error deferral](conformance_svelte.md).
Each is implemented in phase 2 over the Svelte AST, in Svelte-domain terms:

| Rule | Site in `packages/svelte/src/compiler` |
| --- | --- |
| `node_invalid_placement` | `phases/2-analyze/visitors/RegularElement.js:185` |
| `snippet_conflict` | `phases/2-analyze/visitors/SnippetBlock.js:77` |
| `component_invalid_directive` | `phases/2-analyze/visitors/shared/component.js:81` |

The clearest case is the one now closed, `dollar_prefix_invalid`: it is literally
`node.name.startsWith('$')` on a binding — a **reserved-prefix** rule Svelte owns, not
a JS one. `let $$slots = 1;` is valid JavaScript, and tsc — [this repo's oracle for
what is really an error](../CLAUDE.md#strict-mode-only) — accepts it under `--strict`.
Nothing in the deferred set (duplicate parameter names, reserved words as identifiers,
octal escapes, `delete` of a plain name) reaches any of these rules.

The single genuine overlap is `declaration_duplicate`, and Svelte says so itself at
`phases/scope.js:689` ("declaring function twice is also caught by acorn in the parse
phase"). That one row — and only that one — is arguably not tsv's to fix. The other eight
are analysis tsv has not ported.

#### The `validate_assignment` family — closed

**Closed.** Three oracle rules, one refusal, because the oracle itself is one function:
`validate_assignment` (`phases/2-analyze/visitors/shared/utils.js:18-40`, which calls
`validate_no_const_assignment` at `:19`) is reached from `AssignmentExpression.js:11`,
`UpdateExpression.js:11` **and** `BindDirective.js:181`, so an assignment, an update and
a `bind:` ask the same question of the same binding table. tsv ports it in the
whole-component `needs_context` walk (`needs_context.rs`), which already visits every one
of those three positions across the instance script, the module script and the template —
dropped `{:catch}` branches and event handlers included, which is where two of the
suite's samples put the write.

- **Refused**: `assignment to a constant (a const declarator or import local — the oracle's constant_assignment)` — a write to a `const` declarator or import local: a top-level one in either script, equally a NESTED one (a `const` in a block or function body, or a `for (const … of …)` head), and equally a TEMPLATE-scoped one (a `{@const}` name, a `{:then}`/`{:catch}` value, an `{#each}` INDEX — see below), since the oracle's `validate_no_const_assignment` resolves the target through the SCOPE CHAIN rather than against a top-level set. Keyed on the DECLARATION KEYWORD, exactly as the oracle is, so a reactive `const c = $state(0)` refuses too. Only the innermost binding of the name decides — a `let` nested inside a same-named `const` is an ordinary local and its write is accepted. The pattern recursion mirrors `validate_no_const_assignment` exactly: `ArrayPattern` elements and `ObjectPattern` property *values*, and nothing else — a `RestElement`, an `AssignmentPattern` default and a `MemberExpression` match no branch there and are **accepted**, a member target writing *through* the binding rather than rebinding it.
- **Refused**: `assignment to an {#each} item (the oracle's each_item_invalid_assignment)` — a write to an `{#each}` context binding. Block-scoped to the block's body and fallback (the oracle's child scope, `phases/scope.js:1244`/`:1280`), and checked only for a whole-`Identifier` target, both matching the oracle. Runes-only there; this compiler is unconditionally runes-only.
- **Refused**: `assignment to a {#snippet} parameter (the oracle's snippet_parameter_assignment)` — a write to a `{#snippet}` parameter (`phases/scope.js:1342`). NOT runes-gated in the oracle.

**The shadowing over-refusal is closed.** Set membership was name-based where the oracle
is scope-sensitive, so a nested re-declaration sharing a name with a component `const` or
import — an ordinary helper reusing a name for its own local — over-refused. The
`needs_context` walk now carries a **scoped** JS-binding stack (`Nc::js_scope`) beside the
cumulative `shadowed` union: a function parameter and name, a `catch` parameter, a
`for`-head binding, and a nested `let`/`const`/`var`/`class`/function declaration are
pushed at their declaration and popped when their scope closes, and a lookup scans
BACKWARD so the innermost binding of a name decides. Each phase (instance script, module
script, template) additionally rewinds to zero.

⚠️ **Recording a binding is not the same as suppressing the rule.** The stack carries each
entry's KIND, because the two halves of that sentence have different answers. A nested
`let`/`var`/parameter/`catch`/function/class binding carries no oracle rule, so a write to
it is a write to the local and compiles. A nested `const` does not: it is
`declaration_kind: 'const'` to the oracle wherever it sits — `validate_no_const_assignment`
reads the SCOPE CHAIN, not a top-level set — so it carries `constant_assignment` itself and
the write must REFUSE. An earlier form of this scoped set treated every entry uniformly as
"shadowed ⇒ no rule" and so **compiled writes the oracle rejects** — an over-acceptance and
a refusal-contract violation, live-verified on
`const a = 1; function f() { { const a = 0; a = 2; } }` and three siblings. Storing the
kind is what keeps the two apart, and it must stay stored: the two nested orderings have
opposite verdicts (`let a; { const a; a = 1 }` refuses; `const a; { let a; a = 1 }`
compiles, both oracle-verified), so a set that merely knew "some open binding of this name
is const" would get one of them wrong.

The enumeration of declaration FORMS is a **separate** question from the kind one above.
⚠️ An earlier form of this section claimed that incompleteness "fails in the safe
direction — a form the walk does not record leaves no binding, so the write falls through
to the component-level sets and still refuses". **That claim is FALSE, and it is the same
conflation as the one above, one level out**: the fall-through refuses only when the name
is ALSO in a component-level set. When the shadowed-out name is purely LOCAL there is no
component-level entry to fall through to, no rule applies at all, and the write is
ACCEPTED — an over-acceptance whenever that local was a `const`. Two such over-acceptances
were live, both listed by the old text as safe examples, both oracle-verified
(`constant_assignment`):

```svelte
<script>function f(v) { switch (v) { case 1: const w = 1; break; case 2: w = 2; } }</script>
<script>function g() { z = 1; const z = 2; return z; }</script>
```

Both are now **closed**, in the refusing direction only: a `switch` gets ONE block scope
shared by all its cases (the oracle's `SwitchStatement: create_block_scope`,
`phases/scope.js`), and a block's `const` declarations are hoisted into scope before its
statements are walked, mirroring the oracle's scope PRE-PASS (`create_scopes` runs to
completion before any reference is validated, so a write textually earlier than the
`const` still resolves to it). The hoist is deliberately `const`-only: hoisting a `let`,
`class` or function name would be equally faithful to the pre-pass, but those carry no
rule, so recording one earlier could only turn a refusal into an acceptance.

The correct rule for the remaining gaps: **a missing NON-const form is safe** (it carries
no rule either way, so the miss can only over-refuse), **a missing `const` form is a bug**.
Remaining gaps, all of the safe shape — a `var` is scoped to its block rather than hoisted
to its function; a non-`const` declaration (`let`, `class`, a function name) is recorded
where the walk reaches it rather than hoisted; a class EXPRESSION's own name is unrecorded
where a class declaration's is (harmless because the oracle declares a class name `'let'`,
not `const` — `phases/scope.js`'s `ClassDeclaration` visitor). The other direction that
must never occur is a binding OUTLIVING its scope, which would suppress a genuine refusal;
the stack's truncation forecloses it.

**The TEMPLATE-scoped consts are closed too.** A `{@const}` name, a
`{:then}`/`{:catch}` value and the `{#each}` INDEX are all
`declaration_kind: 'const'` to the oracle (`phases/scope.js:1205` via the `ConstTag`
parent test, `:1310`/`:1324`, `:1273`) and were recorded in no set, so a write to one
compiled — the last gap of the UNSAFE shape (`const` to the oracle, unrecorded here;
purely template-local, so nothing falls through to a component-level set and no rule
fires at all). They now enter `Nc::template_consts`, block-scoped at the extent the
oracle's own scope covers: a `{@const}` to its enclosing FRAGMENT (every `Fragment` gets
a child scope, and a fragment holding a declaration tag is never porous —
`1-parse/index.js:306`), a `{:then}`/`{:catch}` value to that branch's fragment, an
`{#each}` index to body + fallback. A fragment's `{@const}` names enter BEFORE any of its
nodes is walked, mirroring the oracle's scope pre-pass, so a write textually earlier than
the `{@const}` refuses too. The set is consulted after `js_scope` (a JS scope always
nests inside a template one, so a handler parameter shadowing a `{@const}` still wins)
and before `each_items`/`snippet_params` — the safe order, since the const rule fires at
any pattern depth while those two fire only on a whole-identifier target. The `bind:`
half closed with it (`BindDirective.js:181` reaches the same validator): a
`bind:this={v}` to a `{:then}` value or a `{@const}` name, previously pinned as a
current-behavior over-acceptance, now refuses as the oracle's `constant_binding`.

⚠️ **The `{#each}` INDEX and the ITEM beside it take DIFFERENT rules**, and conflating
them is a bug in either direction. The item is declared `('each', 'const')` (`:1244`) and
`validate_no_const_assignment` EXCLUDES `kind === 'each'`, so it carries
`each_item_invalid_assignment`; the index is `('template' | 'static', 'const')` (`:1273`)
and carries `constant_assignment`. Both live-verified: the oracle answers "Cannot
reassign or bind to each block argument" for a write to `x` in `{#each xs as x, i}` and
"Cannot assign to constant" for a write to `i` in the same block.

⚠️ **One write position MASKS this rule, and the masking has already been mistaken for a
refutation.** An assignment sitting directly in an emitted template expression
(`{(c = 2)}`) refuses as `mutation inside a template expression` — an unrelated general
rule that fires whatever the target is, `const` or not — so the most natural repro read
green while the residual was fully live. That the rule is target-independent is itself
live-verified: `<script>let n = 0;</script>{(n = 2)}` is COMPILED by the oracle and
refused by tsv under that same message. The two unmasked positions are an event-handler
arrow (`onclick={() => (c = 2)}`) and a write inside a dropped `{:catch}`. Measured over
nine probes (three forms × three positions) before the fix: the oracle rejects all nine
as `constant_assignment`, tsv over-accepted the six unmasked ones; after it, all nine
refuse, and `compile_corpus_compare` now names the tsv-side reason on each, so "tsv also
declines" can be told from "tsv declines for the reason under test" mechanically.

**Closing the shadowing over-refusal did not move corpus parity, and the earlier claim that
it would is falsified.** This section previously recorded that the family cost one parity
point, on the evidence that
`../svelte/packages/svelte/tests/runtime-runes/samples/mutation-local/main.svelte` — a
`const x = localMutation(1)` beside a `function localMutation(input) { let x = input; … x =
2; … }` — was the rule's only corpus member and that "renaming the inner local reaches
parity byte-for-byte, so the file's only blocker is this rule". The rename experiment
**cannot discriminate**: renaming the inner local removes the name collision, which clears
*two* independent name-based residuals at once. Isolating them shows the file has a second
blocker. With the write kept but the template read of `x` removed, the file now reaches
parity (it refused `constant_assignment` before this change) — so the assignment rule is
genuinely closed. With the template read present, it refuses `static evaluation not
portable: binding x is not statically modeled`: the evaluator marks a component binding
`Opaque` when its name appears in `fn_declared`, the whole-component union of names
declared inside any function-like subtree, which exists to compensate for `reassigned`
being shadow-naive. So `mutation-local` moves buckets rather than reaching parity, and the
corpus totals hold at parity **1370** / refused **1041** / 0 MISMATCH / 0 over-acceptance
over 2996 files. The measurement that shows the change fired is the bucket membership, not
the totals: `InvalidAssignmentTarget` goes from one member to **none**, and `static
evaluation not portable` gains exactly that path.

Narrowing that second residual is its own slice, and the scoped set built here is the
substrate for it — `reassigned` is collected at the same two write positions that now
consult `js_scope`, so a write resolving to a local need not mark the component binding.
But relaxing an `Opaque` binding to foldable is the **unsafe** direction (a wrong fold is a
MISMATCH, not an over-refusal), so it wants its own safety analysis rather than a
follow-on edit here.

#### `dollar_prefix_invalid` — closed, and wider than one carve-out

**Closed.** The rule is enforced by the `$`-prefixed binding refusal above
(`Refusal::DollarPrefixedBinding`, `rune_guard.rs`), and the fuzzer's largest
over-acceptance bucket with it: at `--seed 0 --iterations 20000` the fuzzer went from
647 over-acceptances across eleven oracle codes to **435 across nine**, with
`dollar_prefix_invalid` at zero. Mismatches (26) and panics (0) are unchanged.

It is worth recording what the *diagnosis* got wrong, because the shape of the error
recurs. Every one of those 209 mutants was the same instance-script `let $$slots = 1;`,
so the bug read as a single name-keyed carve-out in `walk_expression`'s
`Expression::Identifier` arm — an exemption whose own comment justified it only for a
**reference**, silently licensing a **declaration**. That much was true. The inference
drawn from it — that `$$props`, `$$payload`, `$0` and a bare `$foo` "all refuse
correctly", so the hole was `$$slots`-specific — was **false**, and it was false because
the sample only ever exercised the *declarator* position. Direct probing of the other
binding positions found the same over-acceptance for **every** `$`-prefixed name at a
class-declaration id and an import specifier's local (and at a function-declaration id
whose name is never referenced): those positions had no `$`-prefix check at all, because
the pre-existing refusal fires from the *reference* walk and nothing routed a binding
name through it.

The general lesson: a mutation corpus reports the shapes it *generates*, and a bucket
that is 209-for-209 one shape is evidence about the generator, not about the size of the
hole. Enumerate the rule's positions from the oracle's own visitors and probe each one.

That lesson then recurred **one level up**, which is why the rule needed a second pass.
A post-fix fuzzer run showing `dollar_prefix_invalid = 0` across 20 000 mutants was read
as proof of closure. It was not: a fuzz **zero** is a statement about the generator in
exactly the same way a fuzz **concentration** is, and this generator never crosses
store-name × rune-init × dollar-declarator. Direct probing found the rule still open on
the whole *transform* path — `script_rewrite::rewrite_declaration`, which rewrites a
top-level instance-script declaration rather than guard-walking it. Two halves:

- a declarator whose own init is **not** a rune, when a sibling declarator in the same
  statement **is** (`let a = $state(1), $x = 2;`) — the id went through
  `walk_expression_guarded` under a `WalkCtx` with store reads enabled, so a `$x` whose
  base `x` is any binding (a plain `import { x }` suffices — `svelte/store` is not
  required) took the *store-read* exemption;
- a declarator whose init **is** a rune (`let $x = $state(1);`, `$state.raw`,
  `$derived`) — the id was not walked at all, so no name, bound base or not, was ever
  checked.

The fix moves the rule to the loop's own chokepoint, ahead of the rune dispatch, exactly
where the oracle's `VariableDeclarator` visitor runs `validate_identifier_name` over
every `extract_paths` leaf — so both halves close for every name at once. Confirmation
is by direct probe of both shapes and their boundary variants, not by a fuzz count.

### A CSS ident code point the two parsers disagree on

`U+0085` (`<NEL>`) after a selector name — `.foo\u{0085} { … }` — is an
over-acceptance, and it belongs to a different family from every row above: it is
a **parser** disagreement in `tsv_css`, not a missing analysis-phase rule. Svelte's
CSS parser raises `css_expected_identifier`; tsv's accepts it as an ident
continuation and compiles the component.

The rule tsv implements is the historical one — every code point at or above
`U+0080` is a CSS ident code point. Probing the oracle across the separator family
(`U+00A0`, `U+1680`, `U+2000`, `U+202F`, `U+205F`, `U+3000`, `U+180E`, `U+FEFF`)
shows it accepts all of them and rejects only `U+0085`, so the disagreement is
exactly one code point wide today. Note that css-syntax-3's current
*non-ASCII ident code point* definition is narrower still than either — it
enumerates ranges that deliberately exclude the whitespace-looking separators
(`U+00A0`, `U+2000`–`U+200A`, `U+202F`, `U+205F`, and `U+3000` are all outside it)
— so the oracle is not spec-current here either, and matching the oracle is the
compiler's contract regardless.

Found by `compile_fuzz`'s `exotic_whitespace` operator, which is why the operator
exists: the mutant is `.z\u{0085} + .z` grown out of an ordinary scoping fixture,
and no gate, no fixture, and no corpus file reached it. The fix is in `tsv_css`'s
lexer, so it lands on the parser side rather than in this crate.

### Owed to main

Over-acceptances whose root cause is in a **frontend** crate (`tsv_ts`, `tsv_css`,
`tsv_svelte`), not in `tsv_svelte_compile`. Nothing in this crate can close them —
the dependency runs compiler → frontend, never back — so each is recorded here and
fixed on **main**, graded by a parser-conformance fixture. The two entries above
(the `U+0085` CSS ident code point, and the C5 trailing-`trimEnd` class under
§Mismatch classes under mutation) belong to this family too.

#### `using` / `await using` declarations

`using u = expr;` and `await using u = expr;` in a `<script>` are a standing
**parse-surface** over-acceptance: tsv parses and compiles them, while the pinned
oracle rejects the document outright with `js_parse_error` — its acorn cannot parse
a `using` declaration at all. Probe-verified in both directions (a bare `using`
declarator and one with a later write to the binding; both `js_parse_error`).

Pre-existing and unrelated to the `constant_assignment` rule: the compiler's
scope-stack treatment of a `using` binding (not `const`, per the oracle's exact
`declaration_kind === 'const'` test in `shared/utils.js`) is a source reading only,
and its behavioral half is **undemonstrable** against this oracle — no `using`
document ever reaches the analysis phase. Do not cite oracle behavior for it.

The fix is a `tsv_ts` parser question — whether tsv should accept a declaration
form the canonical parser rejects — and belongs with the parser's
[deliberate early-error deferral](conformance_svelte.md) discussion, not here.

Repro: `printf '<script>function f() { using u = g(); }</script>' > t.svelte &&
cargo run -p tsv_debug compile_corpus_compare t.svelte`.

### Mismatch classes under mutation

`compile_fuzz --seed 0 --iterations 20000` produces **16 mismatches**, classified from
the dumped mutants by diff signature.

⚠️ The count is not comparable across corpus edits. The seed corpus IS
`tests/fixtures_compile`, so adding a fixture changes which mutants are generated;
compare a run only against another run over the same corpus.

**C1 — `{#each}` counter numbering — is CLOSED.** tsv and the oracle disagreed on
which loop got `$$index` vs `$$index_1`/`$$index_2` because tsv allocated both
generated each-block names from one emission-order counter. The oracle allocates them
in two *different* passes, and therefore two different orders:

| name | oracle pass | order | dropped `{:catch}` |
| --- | --- | --- | --- |
| `each_array` | 3-transform, server `EachBlock` visitor (`state.scope.root.unique`) | pre-order | not visited → consumes nothing |
| `$$index` | scope creation, `EachBlock` visitor's trailing `node.metadata = { … }` | **post-order** (assigned *after* body + fallback) | visited → **consumes a name** |

So an `{#each}` nested inside another one's fragment, or sitting in a dropped
`{:catch}`, mis-numbered every later loop. `blocks::assign_each_index_names` now
assigns `$$index` upfront in post-order over the whole fragment tree; `each_array`
stays at emission. Fixtures: `blocks/each_fallback_nested_each`,
`blocks/each_index_after_dropped_catch_each`.

⚠️ Only two of those nestings are reachable today. An `{#each}` in another's **body**
still refuses (`Refusal::NestedEach` — `env.in_each`, a separate gate on the
unvalidated nested *emission* path), so the numbering fix is exercised by an `{#each}`
in a `{:else}` fallback and by one in a dropped `{:catch}`, which is what the two
fixtures cover. The body case is modelled but not yet reachable; it becomes so when
`NestedEach` lifts.

**C2 — the module→instance half of the module-script comment class — is CLOSED.** A
comment in a `<script module>` placed *after* the `<script>` was emitted by the oracle
(into an unrelated template expression) and dropped by tsv. That ordering now refuses
(`Refusal::ModuleCommentAfterInstanceScript`); see
[conformance_svelte_compiler.md](conformance_svelte_compiler.md#module-script-comment-teleported-into-the-instance-script)
for the mechanism, the probed boundary, and why the refusal is coarser than the
mismatch. Zero corpus parity cost.

The residual 16 by diff signature (a clean partition this time — each mutant carries
exactly one):

| Signature | Count | Shape |
| --- | --- | --- |
| `$$props` | 6 | a user `const $$props = 1` where the oracle emits `const $$sanitized_props = 1` (generated-name deconfliction) |
| module-script comment (block-recovered) | 7 | the **other** half of the class, still open — see below |
| generated-function order | 1 | a `<svelte:boundary>` `failed` snippet / hoisted snippet function emitted at a different point in the body than the oracle emits it |
| wrapper | 1 | `$$renderer.component(…)` emitted where the oracle emits none |
| static fold | 1 | tsv folds a `{b}` read the oracle keeps as `$.escape(b)` |

#### The open half: a module comment recovered by a preceding block

Every one of the 7 residual module-comment mismatches is the **same mechanism** as the
closed half — esrap's single comment index being re-seeked backward — reached by a
different route. The trigger boundary, established by probe against the pinned oracle:

> A `<script module>` comment is EMITTED by the oracle when some **earlier** statement in
> the module body contains a `{ … }` block (which carries a `loc`, so opening it re-seeks
> the index back over the comment) **and** some later printed node exists to flush it
> into. tsv drops it → mismatch.

⚠️ **This axis is INDEPENDENT of the closed half's.** The refusal keys on script ORDER
(module after instance); this keys on a PRECEDING BLOCK-BEARING STATEMENT in the module
body. Neither implies the other, so **a two-script document is not covered by the
refusal** — a module script placed *first*, with an instance script present, still
mismatches whenever a block-bearing statement precedes the comment. Live-probed with the
instance template holding a `{x}` read, a `$props()` member read, and no expression at
all; all three emit `// c` on the oracle side and drop it on tsv's. Instance-script
presence is simply not on this axis — do not read the closed half's ordering rule as
covering it.

⚠️ The **flush target** is likewise looser than "a later printed *template* node". A
document with a fully static template (`<p>hi</p>`) mismatches, the target being a later
**module-body** statement (`export const a = 1`). What is confirmed is only that *some*
later printed node is needed, not where it lives.

Confirmed triggering (comment preceded by, and followed by, a statement): a `function`
or `class` declaration, a `const f = function () {}`, a `const f = () => {}`, an object
method `{ m() {} }`, an `if (1) {}`. Confirmed **not** triggering: a plain `const` /
`let` / `var`, an object literal, an array literal, an `import`. A comment before the
module body's **first** statement or after its **last** is also parity — but that was
probed *without* an instance script, and the last-statement carve-out does **not** hold
with one present: the same comment then lands inside the emitted function's parameter
list (`function Input($$renderer // c)`), a mismatch of a different shape. Treat the two
carve-outs as no-instance-script results only.

**Corpus exposure.** A source scan over the `compile:corpus:compare` roots finds 22
module scripts whose body opens a `{` before its first comment — the shape this axis
needs. Spot-checks show them currently **masked**: a probed sample either refuses for an
unrelated reason (a `generics` attribute, `template_emits_nested_block`) or is genuinely
parity. So the corpus being green is evidence about the corpus, not about this hole: the
shape is reachable in ordinary real code and merely shadowed today, and a refusal lifting
elsewhere can unmask it.

Closing it wants the same shape as `template_emits_nested_block` — a blunt
"does any preceding module statement contain a block" scan that deliberately
over-refuses. It is **not** done here: unlike the closed half (zero corpus cost), a
module script holding a function or class beside a comment is ordinary real code, so the
over-refusal's parity cost must be measured before the rule is chosen.

⚠️ **A further class exists but did not come from this run.** `<svelte:head>` ordering —
tsv emits `$.head(…)` *before* the hoisted snippet function where the oracle emits it
after — is a real, hand-confirmed bug. No `--seed 0` mutant contains `<svelte:head>` at
all, so it is tracked separately and must not be counted against a `compile_fuzz` run's
mismatch total.

#### C5 — trailing template whitespace: the source `trimEnd` class

A **sixth** class, produced by `compile_fuzz`'s `exotic_whitespace` operator and
confirmed by hand. A document whose last character is `U+FEFF` or `U+0085` mismatches,
and the two mismatch in **opposite directions**:

| Document | tsv emits | oracle emits |
| --- | --- | --- |
| `<p>a b</p>\u{FEFF}` | `` `<p>a b</p>\u{FEFF}` `` | `` `<p>a b</p>` `` |
| `<p>a b</p>\u{0085}` | `` `<p>a b</p>` `` | `` `<p>a b</p>\u{0085}` `` |

Both directions are one root cause. Svelte's parser opens with
`this.template = template.trimEnd()` (`phases/1-parse/index.js:95`) — JavaScript's
`trimEnd`, i.e. ECMAScript `WhiteSpace` ∪ `LineTerminator`, which **contains** `U+FEFF`
and **excludes** `U+0085`. tsv's counterpart is `trailing_text.trim_end()`
(`crates/tsv_svelte/src/parser/mod.rs:156`) — Rust's `str::trim_end`, i.e. Unicode
`White_Space`, whose disagreement with the JS class is exactly those two code points, one
each way. So the divergence is not a coincidence of two bugs; it is the single
host-vs-target whitespace-class defect this operator exists to find, seen from both sides.

**Scope**, established by probe: **trailing position only** — a leading or mid-document
occurrence is parity, and so is any position *inside* an element (`<p>a\u{FEFF}b</p>`),
which no `trimEnd` reaches. And **only** those two code points: `U+00A0`, `U+2000`,
`U+202F`, `U+3000`, `U+180E`, and `U+200B` are parity in both directions, because they
are either in both classes or in neither.

⚠️ **This is a parser divergence, not a compiler one**, and it is therefore **out of lane
for this branch**. The differing trim is in `tsv_svelte`'s parser and is already visible
in the parse AST — tsv's `Root.fragment` carries a trailing `Text` node for the `U+FEFF`
document and none for the `U+0085` one, and the canonical parser's does the reverse — so
the compiler is faithfully compiling the fragment it is handed. Nothing in
`tsv_svelte_compile` can close it, and `text_class::js_trim` is not the fix: it is
`pub(crate)` to this crate, and the dependency runs `tsv_svelte_compile → tsv_svelte`,
never back. The fix belongs on **main**, in `tsv_svelte`, as a parser-conformance change
graded by a `_svelte_divergence`-class fixture; tracked in
[conformance_svelte.md](conformance_svelte.md).

Repro (either direction): `printf '<p>a b</p>\u{FEFF}' > t.svelte && cargo run -p
tsv_debug compile_compare t.svelte`.

### Snippets and render tags

`{#snippet}` compiles to a `function name($$renderer, ...params) { … }` declaration; `{@render}` to a call.

**Hoisting** (`3-transform/server/visitors/SnippetBlock.js`, `2-analyze/visitors/SnippetBlock.js:37-118`). A `{#snippet}` hoists to its nearest enclosing **block scope** (component root, a block body, a `<svelte:head>` closure), bubbling *through* elements (which share the block's `init`). A **top-level** snippet (a direct child of the root fragment) whose free references all resolve to module scope hoists further, to true module scope (a `function` between the imports and the component); any free reference to an instance binding (a prop, `$state`/`$derived`, or a plain top-level `const`/`let`/`function`/`class` — **imports and globals do not disqualify**) keeps it in the component body. Hoistability is a fixpoint over snippet-to-snippet references (`snippet.rs` ports `can_hoist_snippet` name-based).

- **Supported**: parameter-less and parameter-bearing snippets (destructured params, defaults); **typed** (`: T` / `?`) and **generic** (`<T>`) signatures under `lang="ts"` — both erase (see [TypeScript — Supported](#typescript--supported)); hoistable and body-local; snippets nested in elements/blocks; forward references (`{@render}` before `{#snippet}`); a `new`/prop-rooted access inside a snippet body still drives `needs_context`. Parameters mask to UNKNOWN, so their reads never fold.
- **Refused**: `{#snippet} signature the parser fell back to raw text for` — the signature head (`<T>(params)`) is parsed by wrapping it in a synthetic `function f<T>(params) {}`; when that inner parse fails the AST is empty and the raw text is kept, so there is nothing to erase or emit.
- **Refused**: `{#snippet} with an escaped name` — not reproducible by the name-based port.
- **Refused**: `{#snippet} {name} hoist classification ambiguous` — a referenced name is both an instance binding and a nested (non-parameter) local, so free-vs-shadowed can't be told apart under the flat name model (also an escaped snippet reference).
- **Refused**: `{#snippet} alongside a {@const}/<svelte:head> in the same fragment (hoist order)` — the relative hoist order across kinds isn't reproduced.
- **Refused**: `duplicate {#snippet} {name} (the oracle rejects it)`.
- **Refused**: `{#snippet} rest parameter (the oracle rejects it)` — a **top-level** rest parameter (`{#snippet s(...xs)}`; the oracle's `snippet_invalid_rest_parameter`, raised in its analysis phase). A rest element *nested* inside a destructuring parameter (`{#snippet s({ ...rest })}`) is legal and compiles — the oracle checks only the top level.

`{@render callee(args)}` → `callee($$renderer, ...args)` (or `callee?.($$renderer, …)` when optional). Arguments ride the same value machinery as block tests (a `$derived` read, bare or nested, becomes `d()`; runes/mutations refuse). The trailing `<!---->` anchor (the oracle's `empty_comment`, `RenderTag.js:42`) is emitted unless the enclosing block's sole trimmed child is this render with a **non-dynamic** callee (`clean_nodes` `is_standalone`; a local snippet is non-dynamic, a snippet prop is dynamic). `is_standalone` is inherited by element children, so an element wrapping the render keeps the anchor.

**"A `{@render}` holds a call expression" is a *parse*-time rule** (`render_tag_invalid_expression`, raised while the oracle reads the tag) — so it is decided on the **raw** node, before type erasure. The distinction is observable: `{@render (s as T)(x)}` wraps the *callee* and is still a call, so it compiles; `{@render (s(x) as T)}` and `{@render s(x)!}` wrap the *call* and are rejected, even though erasure would reveal a call underneath. Everything downstream — the callee's identity and the arguments — reads the **erased** node instead, `is_standalone` included (a raw `(s as T)(x)` would otherwise read as a non-identifier callee and emit an anchor the oracle elides).

- **Supported**: `{@render}` of a local snippet (standalone-eligible) or a snippet prop like `{@render children()}` (dynamic — keeps the anchor); optional-chained callee; a TypeScript-wrapped callee.
- **Refused**: `{@render} callee is not a resolvable local snippet or snippet prop` — a member callee (`obj.snip()`), an unresolved identifier, or a non-call expression (the parse-time rule above).

### Components

A **static** component invocation compiles to `Name($$renderer, props)` (`shared/component.js` `build_inline_component`). The callee is the component's name reference; `props` is a plain object literal, or `$.spread_props([…])` when spreads are present. A trailing `<!---->` anchor (`empty_comment`) follows unless the enclosing fragment is standalone — the oracle's `clean_nodes` `is_standalone` (`3-transform/utils.js:285`): a sole non-dynamic component with no `--custom-property` attribute reuses the block's anchor.

**Prop values** (`build_attribute_value`, `is_component = true`): a static text value is the *decoded* data as a string literal (no HTML escape, no trim); a single expression value passes through with **no fold** (a `$derived` read, bare or nested, becomes `d()`); a mixed text+expression value is a template literal with `$.stringify` interpolations, folding to a string literal when every part is statically known. A property key is an identifier when it matches the identifier grammar, else a quoted string; `{ n: n }` prints as `{ n }`. An `on*` handler is an ordinary prop (unlike an element handler, which is dropped). Prop-value expressions feed `needs_context` (a `new`/prop-rooted member/call — including inside a `{...spread}` — wraps the body).

**Default-slot children** compile to an implicit `children: ($$renderer) => { … }` snippet prop plus `$$slots: { default: true }`. The children fragment reuses the normal fragment machinery (whitespace cleaning, control-flow blocks, text-first `<!---->` anchor — the oracle's `is_text_first` Component parent), and the `children` prop appends after the attribute props (into the last props group, or a new one after a trailing spread). An empty or whitespace-only body is not children.

**Named snippet children** (`{#snippet name(…)}`) compile to a `function name($$renderer, …) { … }` declaration in a bare block wrapping the call, a `{ name }` shorthand prop, and a `$$slots: { name: true }` entry (a snippet named `children` keeps the `children` prop but a `default` slot key). Snippets mix with default children — the `children` arrow then sees only the default children (direct `{#snippet}` children are excluded), and `$$slots` carries all keys in source order. A component's snippet children are its own scope: the enclosing block's snippet hoist stops at the component boundary.

- **Supported**: self-closing / prop-only components; string / expression / shorthand / boolean / mixed / non-identifier-key props; consecutive props grouped into objects with `$.spread_props` for spreads; a plain-declaration or import callee; standalone-anchor elision; default-slot children (markup / blocks / expressions / nested components); named snippet children (parameters, prop references).
- **Refused**: `dynamic <{name}> component (member or reactive binding)` — a member component (`<Foo.Bar>`) or a component whose name binding is a prop / `$state` / `$derived` / block-local (the oracle emits an `if (expr) {…}` truthiness guard — a later slice).
- **Refused**: `named slot on <{name}> component` — a `slot="…"` child (grouped into a `$$slots.<name>` closure). A **deliberate** runes-only-fence refusal, not a deferral: this is the *consumer* half of the legacy slot system whose `<slot>` / `<svelte:fragment>` *declaration* half is fenced below, superseded in Svelte 5 by the snippets this compiler already emits. So it is `Refusal::is_deliberate_fence` and sits OUTSIDE the achievable-parity denominator.
- **Refused**: `<{name}> component with both a children prop and default children` — the oracle routes the children to `$$slots.default` with a `children` error.
- **Refused**: `--custom-property attribute on <{name}> component` — the oracle wraps the call in `$.css_props`.
- **Refused**: `bind: directive on <{name}> component` — the oracle emits a `do…while` settle loop.
- **Refused**: `directive on <{name}> component` — a non-`bind:` directive (`use:`/`transition:`/…; mostly oracle-rejected input).
- **Supported**: carried script comments alongside a component invocation — the component call's prop values are template-region borrows, so no comment window sweeps a script comment (see [Comment placement classes](#comment-placement-classes)).

### Attributes

| Shape | Status |
| --- | --- |
| static (`name="value"`, boolean, entity-bearing) | Supported |
| expression (`name={expr}`) → `$.attr(name, expr[, true])` | Supported |
| dynamic `class`/`style` → `$.attr_class` / `$.attr_style` | Supported (unstyled components) |
| mixed text+expression (`"t {a} u"`) with `$.stringify` interpolations | Supported (unstyled components) |
| mixed value whose every part folds statically → a *static* attribute (attr-escape `[&"<]`, folded value verbatim: no trim, no empty-class drop, boolean attributes keep the folded value, null/undefined → `''`; only the chunk-array path folds — a single-expression attribute never does) | Supported |
| event attributes (`on*` single expression) → **dropped** from SSR output (`is_event_attribute`, server `element.js:71`) — decided on the **raw authored name**, case-sensitively (lowercasing is emission-only): `onClick` drops; `ONCLICK`/`oNclick` are NOT events and emit `$.attr('onclick', …)`. The dropped handler still feeds `needs_context`, so a `new`/prop-rooted member or call inside it forces the wrapper | Supported |
| raw `onload`/`onerror` (exact match — `onLoad` on `<img>` is a plain drop) on a load-error element (`img`, `iframe`, `object`, …) → the oracle injects `on{name}="this.__e=event"` capture markup | **Refused**: `event capture attribute on a load-error element` |
| mixed-value raw-`on*` (`onclick="a {b}"`) | oracle-rejected input (`attribute_invalid_event_handler`); tsv refuses `event attribute {name}`. `ONCLICK="a {b}"` is not an event (raw test) and emits through the normal mixed path |
| the no-op drop family on a regular element — `use:` / `transition:` / `in:` / `out:` / `animate:` (with or without an expression / modifiers), and `{@attach expr}` (single or multiple) → **dropped** from SSR output. SSR runs no client lifecycle, so the oracle discards their output (the final discarded `context.visit` in `shared/element.js`). Their expressions are still walked for scope / `needs_context` (a `new`/prop-rooted access inside a `use:` argument or `{@attach}` still forces the wrapper) and still validated (a misplaced rune or a top-level `await` inside the expression refuses); a `use:` / `transition:` / `animate:` *name* is a binding reference that blocks a top-level `{#snippet}` from module-hoisting | Supported |
| `use:` on a load-error element (`img`, `iframe`, `object`, …) — the oracle adds `onload`/`onerror` capture attributes (`events_to_capture`, `shared/element.js`); only `use:` (and a spread) triggers this, the other drop-family kinds still drop cleanly there | **Refused**: `use: directive on a load-error element (event-capture markup not implemented)` |
| conflicting transition directives — the oracle's phase-2 placement check (`transition_duplicate` / `transition_conflict`, `shared/element.js:92-132`): a `transition:` claims both intro and outro, `in:` claims intro only, `out:` claims outro only, and a channel claimed twice is rejected. A single `transition:` / `in:` / `out:`, or an `in:`+`out:` pair, still compiles (each channel claimed at most once); modifiers don't change the direction | **Refused**: `conflicting transition directives (an element may have at most one intro and one outro — the oracle rejects it)` |
| `animate:` outside its one legal position — the oracle's phase-2 placement check (`animation_invalid_placement` / `animation_missing_key` / `animation_duplicate`, `shared/element.js:92-132`): legal only as the **sole** non-trivial child (comments / `{@const}` / declaration tags / whitespace-only text are the trivial siblings) of a **keyed** `{#each}`, and only one per element. A single `animate:` on the sole non-trivial child of a keyed `{#each}` still compiles. Two deliberate over-refusals relative to the oracle, both safe (tsv only refuses *more*, never compiles invalid): (a) an `animate:` in a keyed `{#each}`'s `{:else}` fallback — the oracle checks the **body**'s child count for a fallback element too (`parent.body.nodes`), so it compiles a fallback `animate:` when the body has ≤ 1 non-trivial child; tsv declines to reproduce that quirk; (b) a sibling text node of **non-ASCII** whitespace (VT, NBSP, other Unicode spaces) — tsv's triviality test is ASCII-whitespace, narrower than the oracle's JS `.trim()`, so such a sibling counts as non-trivial | **Refused**: `invalid animate: directive (one per element, only on the sole child of a keyed {#each} — the oracle rejects it)` |
| `class:name={expr}` directive(s) on a **regular element** → the fused `$.attr_class(base, css_hash, { name: expr, … })` call (`build_attr_class`, `shared/element.js`). Base: the authored static `class` value, `$.clsx(expr)` for a dynamic `class={expr}` (per `needs_clsx`), or `''` when there is no authored `class` (the phase-2 synthetic empty-`class` injection, `2-analyze/index.js`). Emitted at the authored `class` attribute's slot, or — synthetic — after all plain attributes (source order preserved). Keys are string literals (the oracle's `b.literal(name)`; `format_canonical` drops the quotes where the name is identifier-safe), values the (erased/guarded/derived-rewritten) directive expressions; a shorthand `class:name` uses the auto-generated same-name identifier (`{ name: name }`, not collapsed). CSS scoping: the element is scoped when a static-class token **or** a `class:` directive name matches a scoped selector — the hash then concatenates into a string-literal base (`(value + ' ' + hash).trim()`) or, for any other base, rides the 2nd argument (`void 0` otherwise) | Supported (regular elements) |
| `class:` alongside a **mixed-value** `class="a {b}"` attribute (the oracle passes the mixed template to `build_attr_class` as the base) | **Refused**: `class: directive alongside a mixed-value class attribute` |
| `style:prop={value}` directive(s) on a **regular element** → the fused `$.attr_style(base, directives)` call (`build_attr_style`, `shared/element.js`) — **two** arguments, no css-hash (style is never scoped). Base: the authored static `style` value, the **bare** expression for a dynamic `style={expr}` (NO `$.clsx`, unlike `class`), or `''` when there is no authored `style` (the phase-2 synthetic empty-`style` injection, `2-analyze/index.js:925`, appended after the synthetic `class`). Emitted at the authored `style` slot, or — synthetic — after all plain attributes (source order preserved). `directives` is a plain object `{ name: value, … }`, or — when any directive carries `\|important` — the 2-element array `[ {normal…}, {important…} ]` (the normal object is `{}` when all are important; source order preserved within each group). Keys lowercase the property name unless it starts with `--` (custom properties keep case), then print as a bare identifier when identifier-safe else a quoted string (`'font-weight'`, `'--MyVar'`); values are the (erased/guarded/derived-rewritten) expression, a static string literal (`style:color="red"`), or — for a shorthand `style:color` — the same-name identifier as object-shorthand `{ color }`. `\|important` routes the property to the important group but does NOT wrap the value | Supported (regular elements) |
| `style:` alongside a **mixed-value** `style="a {b}"` base | **Refused**: `style: directive alongside a mixed-value style attribute` |
| `style:prop="a {b}"` with a **mixed-value** (text + expression) directive value | **Refused**: `style: directive with a mixed-value (text + expression) value` |
| `style:prop\|mod` with an invalid modifier — anything but a single `\|important` (the oracle's `style_directive_invalid_modifier`) | **Refused**: `style: directive with an invalid modifier (only \|important, once, is allowed)` |
| `bind:` core kinds on a **regular element** (the oracle's server `BindDirective` handling, `shared/element.js`): **`bind:this`** → omit (emit nothing; valid on any variable / any element, no `$state` gate) when the (erased) target is a valid bind expression — an Identifier/member chain or a `{get, set}` pair; a non-lvalue target (a call/literal/logical) refuses (`bind_invalid_expression`); **`bind:value`** on `<input>` → `$.attr('value', expr)`; **`bind:checked`** on `<input type="checkbox">` (static) → `$.attr('checked', expr, true)`; **`bind:group`** on `<input>` with a static `type` → a synthesized `$.attr('checked', <synth>, true)`, `<synth>` = `group.includes(<value>)` for `type="checkbox"` else `group === <value>`, where `<value>` is the companion `value` attribute's value (which still emits at its own slot; no companion `value` → the oracle silently drops the bind). Emit only when the (erased) bind target is a `$state`-rooted `Identifier`/member chain (the crate's one supported bindable) | Supported (regular elements) |
| every other `bind:` — a bind on a non-`<input>` target, `value` on `<textarea>`/`<select>`, the `omit_in_ssr` media/dimension/window binds (`clientWidth`, `currentTime`, `files`, …), `bind:open` on `<details>`, the content-editable trio (`innerHTML`/`innerText`/`textContent`), `focused`, an invalid target/type (a dynamic/bare `type` with a two-way bind, a non-checkbox `bind:checked`, a static `type="file"` with `bind:value`), or a bind target that isn't a `$state`-rooted lvalue (a prop, `$derived`, reassigned plain let, a call — a SAFE over-refusal) | **Refused**: `bind: directive {name}` |
| element `{...spread}` (alone, or co-present with `class:` / `style:` / `bind:` / the no-op drop family) → the whole attribute set routes through one fused `$.attributes(object, css_hash, classes, styles, flags)` call (`build_element_spread_attributes` / `prepare_element_spread`, `shared/element.js`), replacing the per-attribute emission with `<name${$.attributes(…)}>`. **object** (1st): source-order properties — a plain attribute → `key: value` (`build_spread_object`: key lowercased then bare identifier or quoted string, `shorthand` when the value is the same-named identifier; value is `build_attribute_value(is_component=false)` — a single Text is HTML-escaped `[&"<]`, a single expression is the bare value (`class` wrapped in `$.clsx` per `needs_clsx`, no fold), a mixed value is a folded string literal (un-HTML-escaped) or a `$.stringify` template, a boolean is `true`), a `bind:` **core kind** → its synthesized `value`/`checked` property at the bind's source slot (the oracle inlines each bind into the object; the slice's `bind:` validity gates still apply — `bind:this` / a no-companion `bind:group` contribute **nothing**, an `omit_in_ssr` bind **refuses** (consistent with the inline path — a safe over-refusal; well-formed `omit_in_ssr`+spread parity is deferred), an invalid target/type/expression **refuses**), a single-expression event handler and `defaultValue`/`defaultChecked` drop, a `{...expr}` → a `...expr` spread element. **css_hash** (2nd): `'svelte-tsvhash'` when the element is scoped — a static-class token **or** a `class:` directive name matches a scoped selector; the hash does **not** concatenate into the class value here, it rides this argument — else elided. **classes** (3rd): the `class:` directives object, the oracle's `b.init(name, expr)` — an **identifier key** (a quoted string literal only when the name isn't identifier-safe, `class:foo-bar` → `{ 'foo-bar': x }`; class names are **case-sensitive**, never lowercased) with **object-shorthand** collapse (`class:active` and `class:active={active}` → `{ active }`, `class:active={x}` → `{ active: x }`); absent with no `class:` directive. **styles** (4th): the `style:` directives object, a **FLAT** `{ prop: value, … }` — **NO `\|important` partitioning** (the divergence from the non-spread `$.attr_style`, which builds the `[ {normal}, {important} ]` array; `\|important` is still *validated* — only a single `\|important` is legal — but does not partition); keys lowercase unless `--`-prefixed, shorthand `style:color` → `{ color }`; absent with no `style:` directive. **flags** (5th): `4` (`ELEMENT_IS_INPUT`) for `<input>`, `2` (`ELEMENT_PRESERVE_ATTRIBUTE_CASE`) for a custom element (hyphenated tag or an `is` attribute), else elided. Trailing absent arguments elide; an interior absent one becomes `void 0`. The no-op drop family contributes nothing to the tag but its expression is still guarded (a stray rune / top-level `await` refuses) | Supported (regular elements) |
| element `{...spread}` co-present with a legacy `on:` event directive or `let:` — the oracle drops both in SSR, but tsv declines to reproduce that (the same over-refusal as on a non-spread element) | **Refused**: `legacy on: directive (runes-only fence)` / `legacy let: directive (runes-only fence)` |
| element `{...spread}` on a `<select>` (the oracle routes it through `$$renderer.select(object, ($$renderer) => …)`, a different callee) / on a load-error element (`img`, `iframe`, …) where a spread triggers `onload`/`onerror` capture markup | **Refused**: `{...spread} on <select> (the oracle routes to $$renderer.select)` / `{...spread} on a load-error element (event-capture markup not implemented)` |
| the legacy directives — a legacy `on:` event directive and `let:`: a **deliberate** runes-only-fence refusal, not a deferral (the oracle compiles `on:` in runes mode, but it's deprecated Svelte-4 syntax — migrate to `onclick`/the runes event attribute) (a `class:`/`style:`/`bind:` alongside one of these still refuses via the sibling; `class:`/`style:`/`bind:` on a **component** refuses through `directive on <{name}> component` / `bind: directive on <{name}> component`) | **Refused**: `legacy on: directive (runes-only fence)` / `legacy let: directive (runes-only fence)` |
| string-literal expression value (`name={'s'}`) | **Refused**: `string-literal expression attribute value (inline-literal path)` |
| dynamic `class`/`style` on a styled component | **Refused**: `dynamic class attribute on a styled component` / `dynamic style attribute on a styled component` / `interpolated {name} attribute on a styled component` |
| `value` attribute on `<textarea>` / `<select>` (child content / `select_value` bookkeeping in the oracle) | **Refused**: `value attribute on <{name}>` |

### Elements

| Shape | Status |
| --- | --- |
| HTML elements, nesting, void elements | Supported. A tag name is **lowercased at emission** when the element sits in the `html` namespace (`RegularElement.js:18` — `context.state.namespace === 'html' ? node.name.toLowerCase() : node.name`); the parser does not normalize case. That one lowered name drives every downstream decision in the oracle's visitor, so `<bR>` lowers to `br`, is therefore VOID, and self-closes with no close tag. A tag in the `svg`/`mathml` namespace keeps its case (`<clipPath>`), and `<foreignObject>` resets its children to `html` so they lower again. `<svelte:element this={…}>` is never lowercased — neither at compile time nor at runtime. |
| components (`<Foo … />`) | Supported (self-closing / prop-only) — see [Components](#components) |
| `<option>` | **Refused**: `<option> (oracle emits $$renderer.option closures)` |
| populated `<select>` / `<optgroup>` | **Refused**: `` <{name}> with children (oracle emits a `<!>` anchor) `` (empty `<select>` is Supported) |
| SVG / MathML | Supported — the fragment's `svg`/`mathml` namespace is inferred (`namespace.rs`, Svelte's `infer_namespace`), so collapsed inter-node whitespace is removed under `svg` (except inside `<text>`), attribute-name case is preserved (`viewBox`), and a spread sets the namespaced `flags`. `<a>`/`<title>` are svg only under an svg ancestor. |
| template-level `<script>` / `<style>` | **Refused**: `template-level <{name}>` |
| children on a void element | **Refused**: `children on void element <{name}>` |
| `<svelte:head>` → `$.head(hash, $$renderer, ($$renderer) => { … })`, hoisted to the fragment front; the head body is a normal fragment (a `<title>` child hoists there and emits its own `$$renderer.title(…)`; other unsupported special children refuse through the usual path). The `hash` is the ported `hash("input.svelte")` (`SvelteHead.js`, Svelte's `utils.js`). | Supported |
| `<svelte:head>` with attributes / sharing a fragment with `{@const}` | **Refused**: `attributes on <svelte:head>` / `<svelte:head> alongside a {@const} in the same fragment (hoist order)` |
| `<title>` (a `TitleElement`, i.e. `<title>` inside `<svelte:head>`) → a `$$renderer.title(($$renderer) => { $$renderer.push(`<title>…children…</title>`) })` statement (`TitleElement.js`). Like `<svelte:head>` it is **hoisted** to its fragment's front (the oracle lists it in `clean_nodes`'s hoisted set and pushes to `state.init`), so it precedes its head siblings regardless of source order and never participates in surrounding whitespace normalization. Its children are `Text`/`ExpressionTag` only, emitted like a regular element's text content (a `{expr}` folds when statically known, else `$.escape(expr)`); its children are **not** whitespace-normalized (the oracle calls `process_children` directly, without `clean_nodes`). Analyzed on the emitted path, so a `new`/prop-rooted access in a title `{expr}` fires the `$$renderer.component` wrapper. | Supported |
| `<title>` with an attribute / a non-text-or-`{expression}` child | **Refused**: `attribute on <title> (the oracle rejects it)` / `invalid <title> content (only text and {expression} — the oracle rejects it)` (`title_illegal_attribute` / `title_invalid_content` — input tsv's permissive parser accepts) |
| `<svelte:window>` / `<svelte:body>` / `<svelte:document>` → emit **nothing** (SSR-inert: their events/binds are client-only, so the oracle produces no template output). A legal one carries only oracle-accepted attributes: a **modern event attribute** (`on*={expr}`), the no-op drop family (`class:`/`style:`/`use:`/`transition:`/`in:`/`out:`/`animate:`/`{@attach}`), and a **whitelisted `bind:`** — the name in the ported `binding_properties` list (`this`/`focused` on any; `innerWidth`/`innerHeight`/`outerWidth`/`outerHeight`/`scrollX`/`scrollY`/`online`/`devicePixelRatio` on window; `activeElement`/`fullscreenElement`/`pointerLockElement`/`visibilityState` on document) **and** its target a reassignable lvalue (`bind:this` any lvalue; every other bind a `$state`-rooted `Identifier`/member — the same fork regular elements use, over-refusing prop/plain-`let` targets as a safe over-refusal). A **top-level** `const`-declared or imported target refuses on every bind path alike (`constant_binding`), via the shared `reassignable_bind_target_root` — including a `const`-declared `$state` (`const c = $state(0)` + `bind:innerWidth={c}`) and a `const`/import `bind:this` target, since the oracle keys that rejection on the declaration keyword, not on reactivity. Writing THROUGH a const binding (`bind:value={o.v}`) stays legal — the oracle's rule tests a bare `Identifier` and lets a member chain fall through. An optional-chained target (`bind:this={o?.el}`) refuses too — acorn wraps such a chain in a `ChainExpression`, which the oracle's `bind_invalid_expression` test rejects. A TEMPLATE-scoped const target — a `{@const}` name, a `{:then}`/`{:catch}` value (`phases/scope.js:1310`/`:1324`), or an `{#each}` index (`:1273`) — refuses on the same terms: each is `declaration_kind: 'const'`, kind `'template'`/`'static'` (not `'each'`), so the oracle raises `constant_binding`. `unassignable_names` is keyed on top-level script statements and has no view of template scopes, so the rule is applied instead by `needs_context`'s `template_consts` scope, which every `bind:` target routes through.) Each surviving expression is guard-dropped (a stray rune / top-level `await` refuses) and still analyzed — a `new`/prop-rooted member/call in a bind or handler fires the `$$renderer.component` wrapper, and a `bind:` marks its target reassigned (a later read of a `$state` target stays dynamic, not folded to its init value). | Supported |
| `<svelte:window>` / `<svelte:body>` / `<svelte:document>` with **oracle-rejected input** — nested (legal only at the component root) / a duplicate of the same kind / children / a spread or a non-event plain attribute / a `bind:` outside the whitelist or with a non-lvalue/const/undefined target | **Refused**: `<{name}> must be a top-level element (the oracle rejects it)` / `duplicate <{name}> element (the oracle rejects it)` / `<{name}> cannot have children (the oracle rejects it)` / `invalid attribute on <{name}> (the oracle rejects it)` / `bind: directive {name}` (`svelte_meta_invalid_placement` / `svelte_meta_duplicate` / `svelte_meta_invalid_content` / `illegal_element_attribute` / `bind_invalid_target`\|`bind_invalid_name`\|`bind_invalid_expression`\|`constant_binding`\|`bind_invalid_value` — all input tsv's permissive parser accepts) |
| `<svelte:window>` / `<svelte:body>` / `<svelte:document>` with a **legacy** `on:` event directive or `let:` | **Refused**: `legacy on: directive (runes-only fence)` / `legacy let: directive (runes-only fence)` (the oracle accepts a legacy `on:` here, but tsv declines it as a deliberate safe over-refusal, matching the regular-element path) |
| `<svelte:element this={…}>` → a statement-level `$.element($$renderer, TAG, attrsFn?, childrenFn?)` call (splits the template push stream like a component; no trailing `<!---->`). **TAG**: `this="div"` → the `'div'` string literal (the parser collapses a mixed `this="a{b}"` to its first static chunk, matching the oracle's legacy warn-and-keep-first); `this={expr}` → the erased expression with a derived read (bare or nested) rewritten to `d()` (no static fold). **attrsFn** (`() => { $$renderer.push(…) }`): the exact regular-element attribute machinery — plain attributes, a `{...spread}` → `$.attributes({…}, css_hash?, classes?, styles?)` (**never** a `flags` argument — the name is always the literal `svelte:element`, so it is never `<input>`/custom), `class:`/`style:` → `$.attr_class`/`$.attr_style` — rendered into a parameterless closure over the enclosing `$$renderer`; elided when it would push nothing. **childrenFn** (`() => { … }`): the element's fragment, emitted like any element child (not text-first, not a component root); elided when empty. The `this={expr}` and every attribute expression are still analyzed — a `new`/prop-rooted access fires the `$$renderer.component` wrapper, and a `this={local}` inside a snippet body blocks module-hoist. | Supported |
| `<svelte:element>` — `bind:this` → **omit** (validate the target is a reassignable lvalue or `{get, set}` pair, then emit nothing — any variable, no `$state` gate; a top-level `const`/import root, and an optional-chained target, refuse via the same shared primitive as the inert elements above — carrying the same open template-scope residual noted there) | Supported |
| `<svelte:element>` with a scoping `<style>` in the component → **CSS-scoped** like a regular element: the element census holds a `<svelte:element>` as a scoping leaf and an ancestor owner, a **type or universal selector matches it unconditionally** (its runtime tag is unknown, `css-prune.js:637-647`) while id/class/attribute selectors route through its real attributes, and `emit_svelte_element` synthesizes the hash class into its attributes closure (folding into an authored `class`/`class:` or, with none, a synthetic `class="svelte-…"`; a spread rides the `css_hash` argument). As a possible sibling it only PROBABLY exists (never triggering the `+` adjacent early-stop, no slot check — `css-prune.js:1041`/`1215`); the `{#each}` self-adjacency wrap-around applies. | Supported |
| `<svelte:element>` with any `bind:` other than `bind:this` (`bind:value`/`checked`/`group`/`innerWidth`/… — oracle-rejected or oracle-emitted), or a `slot="…"` when it is a component child (the oracle routes it to a named slot) | **Refused**: `bind: directive {name}` / `named slot on <{name}> component` (`bind_invalid_target`/`bind_invalid_name` for the invalid binds). The named slot is the **fenced** case — the special-element half of the legacy-slot fence covered above, so `Refusal::is_deliberate_fence` and permanently outside the achievable-parity denominator. `bind:focused` and the `omit_in_ssr` dimension family are the genuinely **deferred** ones: a safe over-refusal (the oracle emits/drops them) awaiting a later sub-slice. |
| `<svelte:element>` with a **legacy** `on:` event directive or `let:` | **Refused**: `legacy on: directive (runes-only fence)` / `legacy let: directive (runes-only fence)` (matching the regular-element path) |
| the legacy special elements (`<slot>`, `<svelte:fragment>`, `<svelte:component>`, `<svelte:self>`) — a **deliberate** runes-only-fence refusal, not a deferral: each is deprecation-warned or superseded by the oracle in Svelte 5 (`<slot>`/`<svelte:fragment>` by the snippets this compiler already emits, `<svelte:component>` by a plain dynamic component reference, `<svelte:self>` by importing the module itself), so they are `Refusal::is_deliberate_fence` and sit OUTSIDE the achievable-parity denominator | **Refused**: `template node special element <{tag}>` — one bucket per kind (`… <slot>`, `… <svelte:fragment>`, `… <svelte:component>`, `… <svelte:self>`) |
| `<svelte:boundary>` — **not** fenced (a first-class Svelte 5 feature), and now emitted. Three shapes, all covered. **No snippet**: an isolated `$$renderer.push('<!--[-->')`, a bare `{ … }` block holding the children, and an isolated `$$renderer.push('<!--]-->')` — spliced straight into the enclosing body, *not* a passthrough (the anchors are real SSR output, and unlike `{#key}`'s marker they never merge into an adjacent sibling's template). **`failed` snippet**: the snippet becomes a `function failed($$renderer, …)` declaration in the enclosing block and the three statements move inside `$$renderer.boundary({ failed }, ($$renderer) => { … })`. **`pending` snippet**: its body REPLACES the children under the `<!--[!-->` opener — but the children are still compiled and discarded, because the oracle visits that fragment unconditionally and a `{#each}` there consumes an `each_array` name a later block must not reuse. ⚠️ Emission order is `failed`-first, VISIT order is children → `pending` → `failed`; the generated names follow the visit order. The children fragment is a block scope (text-first-eligible, `is_standalone` recomputed, `{@const}` legal). `onerror={handler}` is dropped but still guard-walked, like an event handler. | Supported |
| `<svelte:boundary>` with a scoping `<style>` → CSS-scoped. The element census descends the boundary fragment **unconditionally**, including children a `pending` snippet discards: the oracle's CSS pass runs before it decides what to emit, so a selector matching only dropped boundary content is still kept and still scoped. This is the one place the census leaf set is deliberately wider than the emitted set (safe — `element_scope` is a span lookup at emission, so a marked-but-unemitted element contributes nothing). A boundary is **transparent** to the ancestor walk (`div > p` across one matches — `get_ancestor_elements` counts only `RegularElement`/`SvelteElement`) but **opaque** to the upward sibling walk (`b + p` across one does not — the oracle's `is_block` set holds neither `SvelteHead` nor `SvelteBoundary`, so `if (!is_block(current)) break` stops there). | Supported |
| `<svelte:boundary>` with an attribute outside the oracle's closed valid set (`onerror`/`failed`/`pending`) — an unknown plain attribute, a `{...spread}`, or any directive — or with a valid-named attribute whose value is not exactly one `{expression}` (a boolean attribute, a static string, a mixed `a{b}c` value) | **Refused**: `invalid attribute on <svelte:boundary> (the oracle rejects it)` / `non-expression value for <svelte:boundary> attribute {name} (the oracle rejects it)` (`svelte_boundary_invalid_attribute` / `svelte_boundary_invalid_attribute_value` — all input tsv's permissive parser accepts, so each would otherwise be an over-acceptance) |
| `<svelte:boundary>` with the `failed={expr}` / `pending={expr}` **attribute** forms — a deferred gap, not a fence: precedence against a same-named snippet is asymmetric (`failed`: the snippet wins; `pending`: the attribute wins), and a statically-nullish `pending` emits an extra `if`/`else` fork keyed on the evaluator's `is_defined` | **Refused**: `<svelte:boundary> {name}={…} attribute form` |
| `<svelte:options>` | **Refused**: `<svelte:options>` |

#### Validation holes a `<svelte:boundary>` can now reach

Three **pre-existing, general** over-acceptances (tsv compiles what the oracle
rejects) become reachable through a boundary now that it emits rather than
refuses. None is boundary-specific — each fails identically with no boundary in
the document, so the fix belongs with the oracle's whole-component validations,
not with `emit_boundary`:

| Shape | Oracle error | Boundary-free analog that over-accepts identically |
| --- | --- | --- |
| `<svelte:head>` / `<svelte:options>` inside a boundary | `svelte_meta_invalid_placement` | `<div><svelte:head>…`, `{#if true}<svelte:head>…`, `<div><svelte:options …>` |
| `<svelte:boundary onerror={a} onerror={b}>` | `attribute_duplicate` | `<div onclick={a} onclick={b}>` |
| two `{#snippet failed}` (or `pending`) in one boundary | `declaration_duplicate` | `<div>{#snippet a}…{/snippet}{#snippet a}…{/snippet}</div>` |

The last one is why `emit_boundary`'s fragment split takes the first snippet of
each name without refusing a second: the oracle's server visitor does pair
`filter` with `find`, but it never has to choose — scope analysis has already
rejected the duplicate. Scoping a refusal to the boundary would close an
arbitrary sliver of a general hole.

### select-family

A trap for the spread / bind slices: the oracle routes a **children-free**
`<select {...props}>` or `<select bind:value={v}>` through `$$renderer.select(...)`
(a closure form), **not** the ordinary `$.attributes` / `$.attr` attribute path
(`is_select_special` / `is_option_special`, `RegularElement.js`). tsv's existing
"populated `<select>`/`<optgroup>`" refusal catches only the *populated* case, so
the children-free select escapes it. The element-spread slice carries its **own**
select-family refusal (`{...spread} on <select> (the oracle routes to
$$renderer.select)`) rather than fall through to `$.attributes` — a hardwired
first check in `emit_spread_attributes`, before the object is built — and
`bind:value` on `<select>` refuses because the `bind:` slice handles `value` only
on `<input>` (`bind: directive value` for any other target).
`compile_select_family_spread_and_bind_refuse` pins both.

### Styles (CSS scoping)

Selector scoping: a rule's selector is a chain of compounds joined by combinators.
Each compound (type / id / class / attribute / universal simple selectors, plus
trailing non-filtering pseudo-classes/elements) that a successful chain match
reaches gains the deterministic `svelte-tsvhash` class, source-spliced into the
style text (author whitespace preserved) — appended after the compound's last
non-pseudo anchor, or **replacing** a bare `*` — and every element the match touches
gains the class too. Each compound is a kind-tagged predicate list matched JOINTLY
against a candidate element (all predicates must hold on the SAME element); the chain
is matched BACKWARD from the rightmost compound over an **upfront element census** (an
ancestor/sibling path per regular element, since tsv's AST has no upward
navigability), porting the oracle's `apply_selector` / `apply_combinator` /
`relative_selector_might_apply_to_node` / `attribute_matches` (a spread / matching
directive / presence test on a single dynamic expression all "could match"). Per
`ComplexSelector` the first scoped compound takes a plain `.svelte-tsvhash` (a +0-1-0
specificity bump), each later one a zero-specificity `:where(.svelte-tsvhash)`; the
bump resets per comma. A scoped element with no `class` markup synthesizes
`class="svelte-tsvhash"`.

- **Supported**:
  - single, no-combinator compounds of type/id/class/attribute/universal (+ trailing
    pseudo), each matching at least one element;
  - the four **combinators** — descendant ` `, child `>`, next-sibling `+`,
    subsequent-sibling `~` — over those compounds, including a preceding sibling
    reached through a `{#if}` / `{#each}` / `{#await}` / `{#key}` block (block descent)
    and the `{#each}` self-adjacency wrap-around;
  - **basic `:global`**: leading `:global(<compound>) .y` (the `:global(...)` matches
    inside, unscoped, its wrapper stripped; the rest scopes), trailing
    `:global(<compound>)` (dropped from matching by `truncate`, wrapper still
    stripped), a fully-global `:global(<compound>)` (never pruned, scopes nothing),
    and a bare `:global` combinator (`div :global.x` → `div.x`, the preceding
    whitespace eaten).
- **Refused**:
  - `css at-rule in <style>` — every at-rule, including `@keyframes` and `@media`;
  - `nested css rule in <style>` — including a `:global { … }` global block, which is
    a nested rule;
  - `empty css rule in <style> (the oracle comment-wraps it)`;
  - `css combinator selector in <style>` — the `||` column combinator, a combinator
    whose match would cross a `{#snippet}` / `{@render}` site (the site-resolution
    product isn't built — a safe over-refusal), and an empty compound;
  - `unsupported css selector in <style> (:global/:is/:where/:has/:not/:root/nesting)`
    — `:is` / `:where` / `:has` / `:not`, `:root` / `:host`, nesting (`&`), an
    unsupported `:global` form (`:global(a, b)`, `.x:global`, `:global(<chain>)`), a
    bare pseudo-only compound, and namespaced/escaped names;
  - `css attribute selector against a dynamic attribute value (static-eval not ported)`
    — an enumerable dynamic attribute value the oracle's `get_possible_values` would
    enumerate (a safe over-refusal; a single plain expression still assume-matches);
  - `css case-insensitive match with a non-ASCII operand (Unicode case-fold not ported)`;
  - `css selector {selector} matches no element (pruning not implemented)`.
- **Planned** (each its own follow-up sub-slice): `:global { … }` global blocks (a
  nested-rule / comma-list surface) and `@keyframes` name scoping (needs general
  at-rule handling first).

---

## Out-of-Scope Fences

- **Legacy mode**: the compile oracle runs `runes: true`; legacy syntax (`export let`, `$:`, `$$restProps`, …) is oracle-rejected input, not a compile target.
- **Client generation**: **Refused**: `client generation`.
- **Dev mode**: **Refused**: `dev mode output`.
- **Source maps**: not emitted.
- **Custom elements, `svelte:options`-driven modes, async/experimental compiler flags**: not implemented; the corresponding suite inputs surface as refusals or oracle rejections (`experimental_async`).
