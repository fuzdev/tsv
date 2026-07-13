# Svelte Compiler Support (tsv_svelte_compile)

Coverage map for tsv's Svelte-to-JS compiler (`crates/tsv_svelte_compile`): which component shapes compile at oracle parity, which refuse, and which are planned. Companion to the parser/formatter checklists ([checklist_svelte.md](./checklist_svelte.md), [checklist_typescript.md](./checklist_typescript.md), [checklist_css.md](./checklist_css.md)).

## Coverage

The compiler targets **server (SSR) output for runes-mode components**, measured against Svelte's own `compile()` (pinned at **svelte 5.56.4**, the sidecar pin) as the correctness oracle. Parity is judged on the **canonical reprint** of both sides' JS (`canonicalize_js` — an intent-erased reprint, so a byte difference is a real code difference), plus byte-equal CSS.

**The refusal contract**: every component shape is exactly one of

- **Supported** — compiles, and the canonical form matches the oracle's byte-for-byte;
- **Refused** — `compile` returns `CompileError::Unsupported(Refusal)`, a typed refusal from the inventory in `crates/tsv_svelte_compile/src/refusal.rs`, never guessed output;
- **a bug** — both sides compile and the canonical forms differ (`compile_corpus_compare`'s MISMATCH bucket), or generated JS fails its reparse self-validation (`CompileError::CorruptOutput`).

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

tsv ports this as `needs_context.rs`, folding props + imports into a name set. `$effect` forces the wrapper through its own dropped-statement path; `$bindable` is refused by the rune guard. Because the port is name-based where the oracle is scope-sensitive, one shape is genuinely ambiguous and refuses:

- **Refused**: `` member/call rooted at prop/import `{name}` that is also bound in a nested scope (needs_context classification ambiguous) ``

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

A never-updated `$state`/plain binding is statically known and its template reads **fold** into the emitted text (the oracle's evaluator behavior, ported in `analyze.rs`); a bare template read of a non-foldable `$derived` binding becomes a call (`d()`).

**`$$slots` — Supported.** A `$$slots` reference (the oracle's `uses_slots`, detected in the `needs_context` walk) injects `const $$slots = $.sanitize_slots($$props)` as the component function's first statement — before any wrapper — and forces the `$$props` parameter (`transform-server.js:300`). It reads through the rune guard's `$`-prefix refusal by a carve-out; the `$props()` rest injection deconflicts by renaming its destructured prop to `$$slots_` (see the rest-injection section). Component-wide reassignment collection also rides that walk, so a binding mutated inside a dropped event handler is still marked updated (and not folded) — and a name *declared* inside any function-like subtree (a handler param or local) marks the same-named component binding `Opaque`, whose reads refuse (`static evaluation not portable: binding {name} is not statically modeled`): the mutation target may resolve to the shadowing local, so neither folding nor escaping is provable — the script side's exact shadow envelope.

- **Refused**: `comments in a script with a $$slots reference (injected sanitize_slots)` — the injected first statement would sweep the carried-comment windows.

Everything else `$`-shaped refuses (the `rune_guard.rs` exhaustive walk):

- **Refused**: `rune {name}` — any non-sanctioned rune call (`$effect.tracking`, `$inspect`, `$bindable`, `$host`, member-form misuse, a rune call in any non-sanctioned position)
- **Refused**: `$-prefixed identifier {name}` — a bare rune reference (oracle-rejected input) or any `$`-prefixed identifier read
- **Refused**: `read of derived binding {name} (supported only as a bare template expression)`
- **Refused**: `destructuring a $state declarator` / `destructuring a $derived declarator` / `destructuring a $derived.by declarator`
- **Refused**: `binding pattern shape ({kind})` — a `$props()`-family binding whose pattern the analyzer doesn't classify
- **Refused**: `top-level await (async component output not implemented)`

A `$`-prefixed *member name* (`a.$foo`) is not a rune reference and stays compilable.

---

## Script Statements

Instance-script statements are borrowed verbatim (with the rune rewrites applied) into the component function.

- **Supported**: declarations, functions, classes, expression statements, control flow — any statement shape the guard walk covers, with comments carried through losslessly (host-absolute spans).
- **Supported**: `lang="js"` and `lang=""` (compile exactly like no `lang` attribute).
- **Refused**: `instance-script export (component exports / $.bind_props not implemented)` — every export form: the oracle compiles `export const`/`function`/`{ a }` via `$.bind_props`, rejects `export default`/`export let` (runes mode), and drops `export * from`; a verbatim passthrough would nest an `export` inside the component function. A **type-only** export (`export type { X }`, `export interface X {}`, `export declare const x`) erases away before this refusal and compiles.
- **Refused**: `module <script context="module">`
- **Refused**: `` legacy reactive statement `$:` (invalid in runes mode) `` — a **top-level** `$`-labeled statement (the oracle rejects it in runes mode; cloning it through would emit a dead label with no reactivity). A `$` label inside a function, and plain labels anywhere, are ordinary JS the oracle clones through — supported. An escaped top-level label name refuses conservatively (can't be classified from its raw span).
- **Refused**: `import from svelte/internal (forbidden)` — any import whose source starts with `svelte/internal` (the oracle's runes-mode rule; private runtime code)
- **Refused**: `runes-invalid import of {name} from svelte` — a named `beforeUpdate`/`afterUpdate` import from `svelte` (the oracle rejects them in runes mode); an escaped imported name from `svelte` refuses conservatively. A string-literal imported name is skipped exactly as the oracle skips it (its check matches identifier names only).
- **Refused**: `lang="{lang}" instance script (only ts/js supported)` — any `lang` other than `ts`/`js`/empty. The oracle's TypeScript flag tests `lang === 'ts'` **exactly** (case-sensitive), so `lang="typescript"` / `lang="TS"` are plain JS to it; rather than compile them as JS on a guess, tsv refuses.
- **Refused**: `generics attribute on instance script (implies TypeScript)` — an open type-parameter *binding*, not annotation erasure (a separate slice).
- **Refused**: `generated name {name} collides with a user binding` — a user binding named `each_array`/`$$index`-family

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
| typed block patterns — `{#each xs as x: T}`, `{#await p then v: T}`, `{:catch e: T}`, `{@const a: T = v}` | erased at the borrow point (a `{:catch}` binding never reaches output at all — the oracle drops the branch from SSR) |
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

Instance-script comments carry through by default. The classes where the oracle re-anchors comments in ways the span-window model can't reproduce refuse:

- **Refused**: `comment after the last script statement (the oracle re-attaches it into the template)`
- **Refused**: `leading comment glued to the <script> line (no newline before it)`
- **Refused**: `comments with template markup before the script (window ordering)`
- **Refused**: `comment inside a rewritten rune region (dropped by the transform)`
- **Refused**: `comments in a script that uses $derived (not carried through yet)`
- **Refused**: `comments in a script with an argument-less $state()`
- **Refused**: `comments in a script alongside imports (placement around hoisted imports not carried through yet)`
- **Refused**: `comments in a script alongside template blocks (placement not carried through yet)`
- **Refused**: `comments in a script alongside expression-valued attributes`
- **Refused**: `format-ignore directive comment in script`
- **Refused**: `template comments (only instance-script comments are carried through)`

---

## Template

### Static emission — Supported

The oracle's normalization (`3-transform/utils.js:126` `clean_nodes`, `escape_html`), probe-verified: whitespace-only boundary text drops and edge runs trim per fragment; a text edge run abutting a non-text node collapses to one space (text + `{expr}` count as one text); interior whitespace is verbatim; `<pre>`/`<textarea>` preserve everything; entities decode then re-escape (`[&<]` in text, `[&"<]` in static attributes); boolean attributes emit `name=""`; `class`/`style` values collapse+trim; a string-valued `class` that collapses+trims to empty is dropped entirely (static path only — bare `class` keeps `class=""`, empty `style`/`id` stay, a *folded* mixed class keeps `class=""`); void elements close `/>`; a text-first fragment (component root or `{#each}` body — `3-transform/utils.js:295` `is_text_first`) gets a `<!---->` prefix.

### Expressions — Supported

- `{expr}` → `$.escape(expr)`; statically-known values fold as text; a bare derived read becomes `d()`.
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
- **references** — the oracle counts them wherever they sit, so a dropped region's references still drive `needs_context` and block a `{#snippet}`'s module hoist (`attr_refs.rs`'s dropped-fragment view; a `{:catch}` the emitter discards is the reason that view exists).

What a dropped region does **not** get is the *emission* refusals: a directive, a spread, a special element, or a `{@debug}` inside a `{:catch}` compiles, because the oracle drops it too — and neither does the derived-read rule, which is an emission rewrite (`d` → `d()`), not a validity rule. Refusing there would cost parity on shapes the oracle accepts. The `dropped_fragments_are_walked` test pins all three halves.

### Snippets and render tags

`{#snippet}` compiles to a `function name($$renderer, ...params) { … }` declaration; `{@render}` to a call.

**Hoisting** (`3-transform/server/visitors/SnippetBlock.js`, `2-analyze/visitors/SnippetBlock.js:37-118`). A `{#snippet}` hoists to its nearest enclosing **block scope** (component root, a block body, a `<svelte:head>` closure), bubbling *through* elements (which share the block's `init`). A **top-level** snippet (a direct child of the root fragment) whose free references all resolve to module scope hoists further, to true module scope (a `function` between the imports and the component); any free reference to an instance binding (a prop, `$state`/`$derived`, or a plain top-level `const`/`let`/`function`/`class` — **imports and globals do not disqualify**) keeps it in the component body. Hoistability is a fixpoint over snippet-to-snippet references (`snippet.rs` ports `can_hoist_snippet` name-based).

- **Supported**: parameter-less and parameter-bearing snippets (destructured params, defaults); **typed** (`: T` / `?`) and **generic** (`<T>`) signatures under `lang="ts"` — both erase (see [TypeScript — Supported](#typescript--supported)); hoistable and body-local; snippets nested in elements/blocks; forward references (`{@render}` before `{#snippet}`); a `new`/prop-rooted access inside a snippet body still drives `needs_context`. Parameters mask to UNKNOWN, so their reads never fold.
- **Refused**: `{#snippet} signature the parser fell back to raw text for` — the signature head (`<T>(params)`) is parsed by wrapping it in a synthetic `function f<T>(params) {}`; when that inner parse fails the AST is empty and the raw text is kept, so there is nothing to erase or emit.
- **Refused**: `{#snippet} with an escaped name` — not reproducible by the name-based port.
- **Refused**: `{#snippet} {name} hoist classification ambiguous` — a referenced name is both an instance binding and a nested (non-parameter) local, so free-vs-shadowed can't be told apart under the flat name model (also an escaped snippet reference).
- **Refused**: `{#snippet} alongside a {@const}/<svelte:head> in the same fragment (hoist order)` — the relative hoist order across kinds isn't reproduced.
- **Refused**: `duplicate {#snippet} {name} (the oracle rejects it)`.

`{@render callee(args)}` → `callee($$renderer, ...args)` (or `callee?.($$renderer, …)` when optional). Arguments ride the same value machinery as block tests (a bare `$derived` read becomes `d()`; runes/mutations refuse). The trailing `<!---->` anchor (the oracle's `empty_comment`, `RenderTag.js:42`) is emitted unless the enclosing block's sole trimmed child is this render with a **non-dynamic** callee (`clean_nodes` `is_standalone`; a local snippet is non-dynamic, a snippet prop is dynamic). `is_standalone` is inherited by element children, so an element wrapping the render keeps the anchor.

**"A `{@render}` holds a call expression" is a *parse*-time rule** (`render_tag_invalid_expression`, raised while the oracle reads the tag) — so it is decided on the **raw** node, before type erasure. The distinction is observable: `{@render (s as T)(x)}` wraps the *callee* and is still a call, so it compiles; `{@render (s(x) as T)}` and `{@render s(x)!}` wrap the *call* and are rejected, even though erasure would reveal a call underneath. Everything downstream — the callee's identity and the arguments — reads the **erased** node instead, `is_standalone` included (a raw `(s as T)(x)` would otherwise read as a non-identifier callee and emit an anchor the oracle elides).

- **Supported**: `{@render}` of a local snippet (standalone-eligible) or a snippet prop like `{@render children()}` (dynamic — keeps the anchor); optional-chained callee; a TypeScript-wrapped callee.
- **Refused**: `{@render} callee is not a resolvable local snippet or snippet prop` — a member callee (`obj.snip()`), an unresolved identifier, or a non-call expression (the parse-time rule above).

### Components

A **static** component invocation compiles to `Name($$renderer, props)` (`shared/component.js` `build_inline_component`). The callee is the component's name reference; `props` is a plain object literal, or `$.spread_props([…])` when spreads are present. A trailing `<!---->` anchor (`empty_comment`) follows unless the enclosing fragment is standalone — the oracle's `clean_nodes` `is_standalone` (`3-transform/utils.js:285`): a sole non-dynamic component with no `--custom-property` attribute reuses the block's anchor.

**Prop values** (`build_attribute_value`, `is_component = true`): a static text value is the *decoded* data as a string literal (no HTML escape, no trim); a single expression value passes through with **no fold** (a bare `$derived` read becomes `d()`); a mixed text+expression value is a template literal with `$.stringify` interpolations, folding to a string literal when every part is statically known. A property key is an identifier when it matches the identifier grammar, else a quoted string; `{ n: n }` prints as `{ n }`. An `on*` handler is an ordinary prop (unlike an element handler, which is dropped). Prop-value expressions feed `needs_context` (a `new`/prop-rooted member/call — including inside a `{...spread}` — wraps the body).

**Default-slot children** compile to an implicit `children: ($$renderer) => { … }` snippet prop plus `$$slots: { default: true }`. The children fragment reuses the normal fragment machinery (whitespace cleaning, control-flow blocks, text-first `<!---->` anchor — the oracle's `is_text_first` Component parent), and the `children` prop appends after the attribute props (into the last props group, or a new one after a trailing spread). An empty or whitespace-only body is not children.

**Named snippet children** (`{#snippet name(…)}`) compile to a `function name($$renderer, …) { … }` declaration in a bare block wrapping the call, a `{ name }` shorthand prop, and a `$$slots: { name: true }` entry (a snippet named `children` keeps the `children` prop but a `default` slot key). Snippets mix with default children — the `children` arrow then sees only the default children (direct `{#snippet}` children are excluded), and `$$slots` carries all keys in source order. A component's snippet children are its own scope: the enclosing block's snippet hoist stops at the component boundary.

- **Supported**: self-closing / prop-only components; string / expression / shorthand / boolean / mixed / non-identifier-key props; consecutive props grouped into objects with `$.spread_props` for spreads; a plain-declaration or import callee; standalone-anchor elision; default-slot children (markup / blocks / expressions / nested components); named snippet children (parameters, prop references).
- **Refused**: `dynamic <{name}> component (member or reactive binding)` — a member component (`<Foo.Bar>`) or a component whose name binding is a prop / `$state` / `$derived` / block-local (the oracle emits an `if (expr) {…}` truthiness guard — a later slice).
- **Refused**: `named slot on <{name}> component` — a `slot="…"` child (grouped into a `$$slots.<name>` closure).
- **Refused**: `<{name}> component with both a children prop and default children` — the oracle routes the children to `$$slots.default` with a `children` error.
- **Refused**: `--custom-property attribute on <{name}> component` — the oracle wraps the call in `$.css_props`.
- **Refused**: `bind: directive on <{name}> component` — the oracle emits a `do…while` settle loop.
- **Refused**: `directive on <{name}> component` — a non-`bind:` directive (`use:`/`transition:`/…; mostly oracle-rejected input).
- **Refused**: `comments in a script alongside a component invocation` — the component call's minted / borrowed prop-value spans would sweep the carried-comment windows.

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
| directives / spread | **Refused**: `non-plain attribute (directive/spread)` |
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
| SVG / MathML | **Refused**: `<{name}> (foreign namespace)` |
| template-level `<script>` / `<style>` | **Refused**: `template-level <{name}>` |
| children on a void element | **Refused**: `children on void element <{name}>` |
| `<svelte:head>` → `$.head(hash, $$renderer, ($$renderer) => { … })`, hoisted to the fragment front; the head body is a normal fragment (so a `<title>` or other special child refuses through the usual path). The `hash` is the ported `hash("input.svelte")` (`SvelteHead.js`, Svelte's `utils.js`). | Supported |
| `<svelte:head>` with attributes / sharing a fragment with `{@const}` | **Refused**: `attributes on <svelte:head>` / `<svelte:head> alongside a {@const} in the same fragment (hoist order)` |
| other special elements (`<slot>`, `<svelte:window>`, `<title>`, …) | **Refused**: `template node special element` |
| `<svelte:options>` | **Refused**: `<svelte:options>` |

### Styles (CSS scoping)

Minimal scoping: single class selectors gain the deterministic `svelte-tsvhash` class, source-spliced into the style text (author whitespace preserved).

- **Supported**: top-level rules whose selectors are single simple class selectors matching at least one element.
- **Refused**: `css at-rule in <style>`, `nested css rule in <style>`, `css combinator selector in <style>`, ``non-class css selector in <style> (only `.class` is supported)``, `css selector .{class} matches no element (pruning not implemented)`

---

## Out-of-Scope Fences

- **Legacy mode**: the compile oracle runs `runes: true`; legacy syntax (`export let`, `$:`, `$$restProps`, …) is oracle-rejected input, not a compile target.
- **Client generation**: **Refused**: `client generation`.
- **Dev mode**: **Refused**: `dev mode output`.
- **Source maps**: not emitted.
- **Custom elements, `svelte:options`-driven modes, async/experimental compiler flags**: not implemented; the corresponding suite inputs surface as refusals or oracle rejections (`experimental_async`).
