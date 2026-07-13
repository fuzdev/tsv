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
| `$derived(e)` | `$.derived(() => e)` |
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
- **Refused**: `instance-script export (component exports / $.bind_props not implemented)` — every export form: the oracle compiles `export const`/`function`/`{ a }` via `$.bind_props`, rejects `export default`/`export let` (runes mode), and drops `export * from`; a verbatim passthrough would nest an `export` inside the component function.
- **Refused**: `module <script context="module">`
- **Refused**: `lang="{lang}" instance script (type stripping not implemented)` — any `lang` other than `js`/empty
- **Refused**: `generics attribute on instance script (implies TypeScript)`
- **Refused**: `TS enum/module declaration in instance script`
- **Refused**: `generated name {name} collides with a user binding` — a user binding named `each_array`/`$$index`-family

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

### Snippets and render tags

`{#snippet}` compiles to a `function name($$renderer, ...params) { … }` declaration; `{@render}` to a call.

**Hoisting** (`3-transform/server/visitors/SnippetBlock.js`, `2-analyze/visitors/SnippetBlock.js:37-118`). A `{#snippet}` hoists to its nearest enclosing **block scope** (component root, a block body, a `<svelte:head>` closure), bubbling *through* elements (which share the block's `init`). A **top-level** snippet (a direct child of the root fragment) whose free references all resolve to module scope hoists further, to true module scope (a `function` between the imports and the component); any free reference to an instance binding (a prop, `$state`/`$derived`, or a plain top-level `const`/`let`/`function`/`class` — **imports and globals do not disqualify**) keeps it in the component body. Hoistability is a fixpoint over snippet-to-snippet references (`snippet.rs` ports `can_hoist_snippet` name-based).

- **Supported**: parameter-less and parameter-bearing snippets (destructured params, defaults); hoistable and body-local; snippets nested in elements/blocks; forward references (`{@render}` before `{#snippet}`); a `new`/prop-rooted access inside a snippet body still drives `needs_context`. Parameters mask to UNKNOWN, so their reads never fold.
- **Refused**: `typed or generic {#snippet} (implies TypeScript)` — a `<T>` generic or a `: Type`/`?` parameter annotation (oracle-rejected without `lang="ts"`; a `lang="ts"` component is refused at the entry).
- **Refused**: `{#snippet} {name} hoist classification ambiguous` — a referenced name is both an instance binding and a nested (non-parameter) local, so free-vs-shadowed can't be told apart under the flat name model (also an escaped snippet name or reference).
- **Refused**: `{#snippet} alongside a {@const}/<svelte:head> in the same fragment (hoist order)` — the relative hoist order across kinds isn't reproduced.
- **Refused**: `duplicate {#snippet} {name} (the oracle rejects it)`.

`{@render callee(args)}` → `callee($$renderer, ...args)` (or `callee?.($$renderer, …)` when optional). Arguments ride the same value machinery as block tests (a bare `$derived` read becomes `d()`; runes/mutations refuse). The trailing `<!---->` anchor (the oracle's `empty_comment`, `RenderTag.js:42`) is emitted unless the enclosing block's sole trimmed child is this render with a **non-dynamic** callee (`clean_nodes` `is_standalone`; a local snippet is non-dynamic, a snippet prop is dynamic). `is_standalone` is inherited by element children, so an element wrapping the render keeps the anchor.

- **Supported**: `{@render}` of a local snippet (standalone-eligible) or a snippet prop like `{@render children()}` (dynamic — keeps the anchor); optional-chained callee.
- **Refused**: `{@render} callee is not a resolvable local snippet or snippet prop` — a member callee (`obj.snip()`) or an unresolved identifier.

### Components

A **static** component invocation compiles to `Name($$renderer, props)` (`shared/component.js` `build_inline_component`). The callee is the component's name reference; `props` is a plain object literal, or `$.spread_props([…])` when spreads are present. A trailing `<!---->` anchor (`empty_comment`) follows unless the enclosing fragment is standalone — the oracle's `clean_nodes` `is_standalone` (`3-transform/utils.js:285`): a sole non-dynamic component with no `--custom-property` attribute reuses the block's anchor.

**Prop values** (`build_attribute_value`, `is_component = true`): a static text value is the *decoded* data as a string literal (no HTML escape, no trim); a single expression value passes through with **no fold** (a bare `$derived` read becomes `d()`); a mixed text+expression value is a template literal with `$.stringify` interpolations, folding to a string literal when every part is statically known. A property key is an identifier when it matches the identifier grammar, else a quoted string; `{ n: n }` prints as `{ n }`. An `on*` handler is an ordinary prop (unlike an element handler, which is dropped). Prop-value expressions feed `needs_context` (a `new`/prop-rooted member/call — including inside a `{...spread}` — wraps the body).

- **Supported**: self-closing / prop-only components; string / expression / shorthand / boolean / mixed / non-identifier-key props; consecutive props grouped into objects with `$.spread_props` for spreads; a plain-declaration or import callee; standalone-anchor elision.
- **Refused**: `dynamic <{name}> component (member or reactive binding)` — a member component (`<Foo.Bar>`) or a component whose name binding is a prop / `$state` / `$derived` / block-local (the oracle emits an `if (expr) {…}` truthiness guard — a later slice).
- **Refused**: `<{name}> component with children` — the implicit `children` snippet prop and named snippet props are a later slice (an empty or whitespace-only body is not children).
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
