# Svelte Compiler Support (tsv_svelte_compile)

Coverage map for tsv's Svelte-to-JS compiler (`crates/tsv_svelte_compile`): which component shapes compile at oracle parity, which refuse, and which are planned. Companion to the parser/formatter checklists ([checklist_svelte.md](./checklist_svelte.md), [checklist_typescript.md](./checklist_typescript.md), [checklist_css.md](./checklist_css.md)).

## Coverage

The compiler targets **server (SSR) output for runes-mode components**, measured against Svelte's own `compile()` (pinned at **svelte 5.56.4**, the sidecar pin) as the correctness oracle. Parity is judged on the **canonical reprint** of both sides' JS (`canonicalize_js` — an intent-erased reprint, so a byte difference is a real code difference), plus byte-equal CSS.

**The refusal contract**: every component shape is exactly one of

- **Supported** — compiles, and the canonical form matches the oracle's byte-for-byte;
- **Refused** — `compile` returns `CompileError::Unsupported(Refusal)`, a typed refusal from the inventory in `crates/tsv_svelte_compile/src/refusal.rs`, never guessed output;
- **a bug** — both sides compile and the canonical forms differ (`compile_corpus_compare`'s MISMATCH bucket), generated JS fails its reparse self-validation (`CompileError::CorruptOutput`), or a TypeScript-only node survives type erasure (`CompileError::TypeErasureLeak`).

Inputs the oracle itself rejects (legacy-mode syntax, invalid JS, TypeScript in a plain script) are out of scope for parity — the corpus runner buckets them ORACLE_REJECTED.

The **Refused** entries below quote each `Refusal`'s stable **bucket key** (user-chosen identifiers shown as `{name}`); `compile_corpus_compare` reports these keys directly (via `Refusal::bucket_key`), so this document maps one-to-one onto corpus runs. Each variant also carries a human-readable `Display` message that substitutes the real name — the two are deliberately decoupled so a message can be reworded without shifting corpus buckets.

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

- **Refused**: `` member/call rooted at prop/import `{name}` that is also bound in a nested scope (needs_context classification ambiguous) ``
- **Refused**: `member/call rooted at an escaped identifier (classification not ported)` — the root's name can't be read from its raw span.

### `$$props` coupling — Supported

The component function signature is `($$renderer)` or `($$renderer, $$props)`: the oracle injects the `$$props` parameter when `should_inject_context` fires or the component declares props (`should_inject_props`, `transform-server.js:313-326`). tsv reproduces both the wrapper and the parameter coupling.

### Multi-declarator split — Supported

A multi-declarator **top-level instance declaration** splits into one declaration per declarator, source order preserved — the oracle's "one declarator per declaration" normalization (`2-analyze/index.js:1148`, `2-analyze/index.js:1154`). Nested declarations (function bodies, `for` heads) stay joined.

- **Refused**: `comments in a script alongside a multi-declarator declaration (the oracle re-anchors comments inside the split)`

### `$props()` rest injection — Supported

A rest element in the `$props()` object pattern gains `$$slots, $$events` immediately before it; a non-destructured `let props = $props()` becomes `let { $$slots, $$events, ...props } = $$props` (`3-transform/server/visitors/VariableDeclaration.js:60-77`). A plain destructure without a rest gets no injection. When the component also references `$$slots` (so the sanitize_slots const owns that name), the injected prop deconflicts by renaming — `$$slots: $$slots_` (`VariableDeclaration.js:56-73`; always the `_` suffix; `$$events` never renames; a user `$$slots_`/`$$events` reference or declaration is oracle-rejected input, so no second-order collision exists).

- **Refused**: `$props() binding pattern (not an identifier or object pattern — the oracle rejects it)`
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

**`$$slots` — Supported.** A `$$slots` reference (the oracle's `uses_slots`, detected in the `needs_context` walk) injects `const $$slots = $.sanitize_slots($$props)` as the component function's first statement — before any wrapper — and forces the `$$props` parameter (`transform-server.js:300`). It reads through the rune guard's `$`-prefix refusal by a carve-out; the `$props()` rest injection deconflicts by renaming its destructured prop to `$$slots_` (see the rest-injection section). Component-wide reassignment collection also rides that walk, so a binding mutated inside a dropped event handler is still marked updated (and not folded) — and a name *declared* inside any function-like subtree (a handler param or local) marks the same-named component binding `Opaque`, whose reads refuse (`static evaluation not portable: binding {name} is not statically modeled`): the mutation target may resolve to the shadowing local, so neither folding nor escaping is provable — the script side's exact shadow envelope.

- **Refused**: `comments in a script with a $$slots reference (injected sanitize_slots)` — the injected first statement would sweep the carried-comment windows.

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
- **Refused**: `read of derived binding {name} (unsupported read position)` — a `$derived` read (bare or nested) rewrites to `d()` in both template value positions and script positions (above), so this refuses only the positions no rewrite reaches: a **template pattern default** (`{#each xs as {v = d}}` — the oracle emits bare `d`, a deferred safe over-refusal; `{#await p then {x = d}}` — the oracle emits `d()`, so refusing is mandatory), a read under an **unsupported wrapper** (an object literal, an arrow, a tagged template), an **escaped-identifier** derived read (`{d}` — classification not ported; refused rather than emit bare `d`), and a `$derived` name **shadowed** by a nested-scope local (`DerivedReadShadowed`, a safe over-refusal for the name-based rewrite)
- **Refused**: `destructuring a $state declarator` / `destructuring a $state.snapshot declarator` / `destructuring a $derived declarator` / `destructuring a $derived.by declarator`
- **Refused**: `binding pattern shape ({kind})` — a `$props()`-family binding whose pattern the analyzer doesn't classify
- **Refused**: `top-level await (async component output not implemented)`

A `$`-prefixed *member name* (`a.$foo`) is not a rune reference and stays compilable.

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
- **Refused**: `lang="{lang}" script (only ts/js supported)` — any `lang` other than `ts`/`js`/empty (on the instance **or** module script). The oracle's TypeScript flag tests `lang === 'ts'` **exactly** (case-sensitive), so `lang="typescript"` / `lang="TS"` are plain JS to it; rather than compile them as JS on a guess, tsv refuses.
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
- **Refused**: `static evaluation not portable: {reason}` / `static fold not portable: {reason}` — a statically-known value whose byte-exact stringification isn't proven (the evaluator's bounded domain)

### Blocks

| Block | Status |
| --- | --- |
| `{#if}` / `{:else if}` / `{:else}` | Supported (flat chain, numbered anchors, synthesized terminal else) |
| `{#each}` (with `{:else}`, authored index, sibling numbering) | Supported |
| nested `{#each}` | **Refused**: `` nested {#each} (the oracle's unique-name allocation order is not reproducible) `` |
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

**Known open holes** on the validation axis — both over-acceptances (tsv compiles what the oracle rejects), neither corpus-reachable:

| Dropped construct | Emitted partner | Oracle error |
| --- | --- | --- |
| `{$$slots.x}` | `{@render …}` | `slot_snippet_conflict` |
| `{#snippet s()}` | `export { s }` from a module script | `snippet_invalid_export` |

Neither `$$slots` nor `{#snippet}` is a fence — tsv intends to support both — so closing these means porting the oracle's whole-component validations, not widening the presence match. That is separate work from the dropped-region walk.

**Everything else keeps compiling** in a dropped `{:catch}`: `<svelte:component>`, `<svelte:self>` (under an `{#if}`), `<svelte:fragment>` and a `slot="…"` component child (both as a component's children), plus the unfenced `<svelte:element>` and `<svelte:boundary>`. That set is clean on both axes — verified by reading the writers, not by probing: the whole-component fields a phase-2 validation reads (`slot_names`, `uses_slots`, `uses_render_tags`, `event_directive_node`, `uses_event_attributes`, `snippets`) are written only by `SlotElement` / an `$$slots` `Identifier` / `RenderTag` / `OnDirective` / an event `Attribute` / `SnippetBlock`, and none of those constructs is one of them. Refusing them to make the fence uniform would trade correct output for nothing. `let:` is likewise on neither axis (its only check, `let_directive_invalid_placement`, is local to its parent) but refuses anyway, to keep the fenced `on:`/`let:` pair in one census bucket. Only the placement-restricted metas (`<svelte:head>`, `<svelte:window>`, `<svelte:body>`, `<svelte:document>`) are unreachable, rejected by the oracle inside any block.

`dropped_fragments_are_walked` pins the expression halves; `dropped_fragment_refuses_presence_read_nodes` pins both presence axes **and** the must-not-over-refuse set beside them.

#### `analysis.elements` — presence-read, currently safe by accident

`RegularElement.js` and `SvelteElement.js` push **every** element into `analysis.elements`, which drives CSS pruning (`2-analyze/index.js` → `prune(analysis.css.ast, analysis.elements)`). An element in a `{:catch}` therefore keeps a CSS rule alive in the oracle's output. tsv's element census excludes `{:catch}`, so today it **refuses** such a component (`css selector … matches no element`) — safe by containment, since tsv's element set is a subset of the oracle's and a smaller set can only over-refuse.

⚠️ That safety is incidental, not designed. **The moment CSS pruning is implemented, `{:catch}` elements must enter the census** — otherwise the refusal disappears and the under-count becomes a silent CSS mismatch instead.

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
| HTML elements, nesting, void elements | Supported |
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
| `<svelte:window>` / `<svelte:body>` / `<svelte:document>` → emit **nothing** (SSR-inert: their events/binds are client-only, so the oracle produces no template output). A legal one carries only oracle-accepted attributes: a **modern event attribute** (`on*={expr}`), the no-op drop family (`class:`/`style:`/`use:`/`transition:`/`in:`/`out:`/`animate:`/`{@attach}`), and a **whitelisted `bind:`** — the name in the ported `binding_properties` list (`this`/`focused` on any; `innerWidth`/`innerHeight`/`outerWidth`/`outerHeight`/`scrollX`/`scrollY`/`online`/`devicePixelRatio` on window; `activeElement`/`fullscreenElement`/`pointerLockElement`/`visibilityState` on document) **and** its target a reassignable lvalue (`bind:this` any lvalue; every other bind a `$state`-rooted `Identifier`/member — the same fork regular elements use, over-refusing prop/plain-`let` targets as a safe over-refusal; one **known residual** over-acceptance runs the other way, shared with the regular-element bind path — a `const`-declared `$state` root (`const c = $state(0)` + `bind:innerWidth={c}`, or a `const` `bind:this` target) still compiles here where the oracle rejects it (`constant_binding`), pending reassignability tracking on the binding table). Each surviving expression is guard-dropped (a stray rune / top-level `await` refuses) and still analyzed — a `new`/prop-rooted member/call in a bind or handler fires the `$$renderer.component` wrapper, and a `bind:` marks its target reassigned (a later read of a `$state` target stays dynamic, not folded to its init value). | Supported |
| `<svelte:window>` / `<svelte:body>` / `<svelte:document>` with **oracle-rejected input** — nested (legal only at the component root) / a duplicate of the same kind / children / a spread or a non-event plain attribute / a `bind:` outside the whitelist or with a non-lvalue/const/undefined target | **Refused**: `<{name}> must be a top-level element (the oracle rejects it)` / `duplicate <{name}> element (the oracle rejects it)` / `<{name}> cannot have children (the oracle rejects it)` / `invalid attribute on <{name}> (the oracle rejects it)` / `bind: directive {name}` (`svelte_meta_invalid_placement` / `svelte_meta_duplicate` / `svelte_meta_invalid_content` / `illegal_element_attribute` / `bind_invalid_target`\|`bind_invalid_name`\|`bind_invalid_expression`\|`constant_binding`\|`bind_invalid_value` — all input tsv's permissive parser accepts) |
| `<svelte:window>` / `<svelte:body>` / `<svelte:document>` with a **legacy** `on:` event directive or `let:` | **Refused**: `legacy on: directive (runes-only fence)` / `legacy let: directive (runes-only fence)` (the oracle accepts a legacy `on:` here, but tsv declines it as a deliberate safe over-refusal, matching the regular-element path) |
| `<svelte:element this={…}>` → a statement-level `$.element($$renderer, TAG, attrsFn?, childrenFn?)` call (splits the template push stream like a component; no trailing `<!---->`). **TAG**: `this="div"` → the `'div'` string literal (the parser collapses a mixed `this="a{b}"` to its first static chunk, matching the oracle's legacy warn-and-keep-first); `this={expr}` → the erased expression with a derived read (bare or nested) rewritten to `d()` (no static fold). **attrsFn** (`() => { $$renderer.push(…) }`): the exact regular-element attribute machinery — plain attributes, a `{...spread}` → `$.attributes({…}, css_hash?, classes?, styles?)` (**never** a `flags` argument — the name is always the literal `svelte:element`, so it is never `<input>`/custom), `class:`/`style:` → `$.attr_class`/`$.attr_style` — rendered into a parameterless closure over the enclosing `$$renderer`; elided when it would push nothing. **childrenFn** (`() => { … }`): the element's fragment, emitted like any element child (not text-first, not a component root); elided when empty. The `this={expr}` and every attribute expression are still analyzed — a `new`/prop-rooted access fires the `$$renderer.component` wrapper, and a `this={local}` inside a snippet body blocks module-hoist. | Supported |
| `<svelte:element>` — `bind:this` → **omit** (validate the target is a reassignable lvalue or `{get, set}` pair, then emit nothing — any variable, no `$state` gate; carries the same shared `const`-`$state` residual noted for the inert elements above) | Supported |
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
