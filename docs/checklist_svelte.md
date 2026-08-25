# Svelte Language Support

Comprehensive reference for Svelte template syntax features supported by tsv's parser and formatter.

## Coverage

All Svelte 5.x template syntax features are supported, as enumerated below; parse conformance is measured against Svelte's parser on the fixture suite and corpus (see [conformance_svelte.md](./conformance_svelte.md)). Experimental features that require compiler flags are listed under [Experimental Async](#experimental-async--parseformat-supported) — tsv parses and formats them regardless of the flag.

**Spec References**:

- Svelte docs: `../../svelte/documentation/docs/`
- Compiler source: `../../svelte/packages/svelte/src/`
- Existing fixtures: `tests/fixtures/svelte/`

---

# Supported Features

## Elements

### HTML Elements

- Block elements (`<div>`, `<p>`, `<section>`)
- Inline elements (`<span>`, `<a>`, `<strong>`)
- Void elements (`<br>`, `<input>`, `<img>`, `<hr>`)
- Self-closing syntax (`<div />`) — prettier divergence: tsv expands per Svelte warning
- Nested elements (multi-level)

### SVG Elements

- SVG namespace (`<svg>`, `<path>`, `<rect>`)
- SVG attributes (`viewBox`, `d`, `fill`)

### MathML Elements

- MathML namespace (`<math>`, `<mi>`, `<mrow>`)

### Components

- PascalCase components (`<Component />`)
- Dot notation (`<my.Component />`)
- Self-closing components
- Components with children
- Nested components

### Whitespace

- Block element spacing (blank lines preserved)
- Inline element spacing (whitespace normalized)
- Pre-sensitive whitespace (`<pre>`, `<textarea>`)
- Text node normalization
- Leading/trailing whitespace handling

---

## Attributes

### Basic Attributes

- Standard attributes (`name="value"`)
- Empty string values (`attr=""`)
- Boolean attributes (`disabled`, `checked`)
- Names with non-identifier chars (`a%b`, directive `on:click%x`) — read up to `[\s=/>"']`, mirroring Svelte's `read_tag`

### Dynamic Attributes

- Expression attributes (`name={expr}`)
- Mixed text+expression (`"text{expr}text"`)
- Shorthand attributes (`{variable}`) — the interior goes through the same `read_identifier` as the block-head and block-binding positions, so all of its rules apply: the ECMAScript `ID_Start`/`ID_Continue` classes (`{℘}` valid, `{a²}` not), an empty name (`{123}`, `{1a}`, `{²}`), and a reserved word (`{this}`, `{class}`). See `attributes/{shorthand_numeric_invalid,shorthand_reserved_invalid,shorthand_unicode_identifier}` and, for the whole 48-word list across all six `read_identifier` positions, `tests/svelte_read_identifier.rs`
- Spread attributes (`{...object}`)
- Multiple spread attributes

### Quote Handling

- Double quotes (`"value"`)
- Single quotes (`'value'`) — normalizes to double quotes
- Unquoted (HTML-valid, rare)

### Special Characters

- HTML entities in attributes (`&amp;`, `&quot;`)
- Escape sequences (`\n`, `\t`)
- Unicode escapes

---

## Text Content

### Basic Text

- Plain text
- Whitespace preservation rules
- Line break handling

### Expression Tags

- Single expression (`{expr}`)
- Multiple expressions
- Expressions in text context
- Expressions in attribute context
- Nested ternary expressions

### HTML Entities

- Named entities (`&nbsp;`, `&amp;`)
- Decimal numeric (`&#123;`)
- Hex numeric (`&#x7B;`)
- Brace escapes (`&lbrace;`, `&rbrace;`)

### Escape Sequences

- Backslash escapes in strings
- Unicode codepoint escapes
- Surrogate pairs
- Combining characters

---

## Control Flow Blocks

### If Blocks

- Basic if (`{#if cond}...{/if}`)
- Else branch (`{:else}`)
- Else-if branch (`{:else if cond}`)
- Else-if chains (multiple)
- A block's alternate is filled once — a second `{:else}`, or an `{:else if}` following an `{:else}`, is rejected. Svelte's reader replaces `block.alternate` unguarded and loses the first branch's markup; tsv declines that content loss. See [conformance_svelte.md](./conformance_svelte.md) §Block Continuation Corrections
- Nested if blocks
- If with expressions only
- If with mixed content

### Each Blocks

- Basic each (`{#each items as item}`)
- With index (`{#each items as item, i}`)
- With key (`{#each items as item (item.id)}`)
- With index and key (`{#each items as item, i (key)}`)
- Each else (`{:else}`) — filled once, like an if block's alternate: a second `{:else}` is rejected rather than replacing `block.fallback`. See [conformance_svelte.md](./conformance_svelte.md) §Block Continuation Corrections
- Destructuring - object (`{#each items as { a, b }}`) — spaced braces match prettier; the lone divergence is the empty pattern (`{}`), see [conformance_prettier_svelte.md](./conformance_prettier_svelte.md)
- Destructuring - array (`{#each items as [a, b]}`)
- Destructuring with rest (`{#each items as {a, ...rest}}`)
- Destructuring with defaults (`{#each items as {a = 1}}`) — prettier divergences: literal defaults normalize (single quotes + numeric form), and a renamed property keeps its key where prettier drops it. See [conformance_prettier_svelte.md](./conformance_prettier_svelte.md)
- Typed context binding (`{#each items as item: number}`, lang="ts")
- Typed destructured context binding (`{#each items as { a }: { a: number }}`, `{#each pairs as [n]: [number]}`) — the annotation attaches to the pattern; the wire `end` widens past it while `loc` does not, matching Svelte's `read_pattern`. See [conformance_svelte.md](./conformance_svelte.md)
- Each without `as` (`{#each items}`, `{#each items, i}`, `{#each items, i (key)}`) — index/key are valid without a context binding; all route through the same index/key parser as the `as` form
- Each whose head holds an `as` but no binding (`{#each items as A satisfies B}`, lang="ts") — Svelte unwinds a head assertion only when the expression's **outermost** node is a `TSAsExpression`, so a run ending on `satisfies` keeps the whole run as the iterable and the block is binding-less. A later `satisfies` cancels an earlier `as` the same way (`{#each items as A[] as item satisfies B}` — `item` is a type). Routes through the no-`as` index/key tail, so the two spellings of "no binding" produce one shape. See `blocks/each/type_assertion_satisfies_no_binding/`
- Nested each blocks
- Binding ends at `}` — a stray comment, leftover index/key fragment, or junk after the binding is rejected (matching Svelte's final `eat('}')`), never silently dropped. Index must be a bare identifier; the key `(…)` is matched with the trivia-aware bracket scanner. See `blocks/each/{no_as_with_index_key, with_index_key/input_invalid_*}`
- A **plain-identifier** binding is read by Svelte's `read_identifier`, so a reserved word (`{#each items as eval}`) is rejected — the same rule as the index and a `{#snippet}` name, since `read_pattern` opens with that call. Only the **destructuring** branch (`{#each items as { eval }}`) falls through to acorn, where the strict-mode early error answers instead. See `blocks/head_reserved_identifier/` and `tests/svelte_read_identifier.rs`

### Await Blocks

- Basic await (`{#await promise}...{/await}`)
- Pending content
- Then clause (`{:then value}`)
- Catch clause (`{:catch error}`)
- Shorthand then (`{#await promise then value}`)
- Shorthand catch (`{#await promise catch error}`)
- Destructuring in `then`/`catch` bindings (`{:then {a = 1}}`) — same brace-hugging + default-value divergences as each blocks
- Typed `then`/`catch` value (`{:then value: number}`, `{:catch error: Error}`, lang="ts") — including a destructured pattern (`{:then { a }: { a: number }}`), which carries the same `end`/`loc` asymmetry as a typed each binding
- `then`/`catch` value is a bare pattern — a comment immediately before it, or in either gap around its annotation (between the bare pattern and its `:` — or `}` when untyped — and between the annotation and the `}`), is rejected (matching Svelte's `read_pattern` + `read_type_annotation`, which cross those gaps with `allow_whitespace` alone), never relocated or dropped; a comment *inside* a destructure (`{a /* c */}`) or *inside* the type (`value: /* c */ number`) stays valid. See `blocks/await/{then_shorthand,then,catch_shorthand,catch}/input_invalid_*_comment` and `blocks/await/binding_annotation_comment_prettier_divergence/input_invalid_*`
- Each clause is filled once — a repeated `{:then}` or `{:catch}` is rejected (Svelte's `block_duplicate_clause`) rather than overwriting the earlier fragment, in the full form and after either shorthand head. See `blocks/await/{then_catch,then_shorthand,catch_shorthand,then_shorthand_catch}/input_invalid_duplicate_*`, with the error wording pinned by `tests/svelte_block_continuation_clause.rs`
- A **plain-identifier** `{:then}` / `{:catch}` binding takes the reserved-word rule, like an `{#each}` binding — both are `read_pattern` positions, and only its destructuring branch defers to acorn
- Nested await blocks

### Key Blocks

- Basic key (`{#key expr}...{/key}`)
- Key with component
- Nested key blocks

### Mixed Control Flow

- If in each
- Each in if
- Await in each
- Deep nesting (3+ levels)

---

## Template Tags

### Expression Tags

- Basic expression (`{expr}`)
- Complex expressions
- Optional chaining in expressions
- Regex literals with parentheses

### HTML Tag

- Basic html (`{@html expr}`)
- HTML with long content

### Const Tag

- Basic const (`{@const x = value}`)
- Const with destructuring
- Const in various contexts (if, each, await)
- Own line in its fragment, except when glued to content on both sides (shared with the
  declaration tag — see [conformance_prettier_svelte.md §Svelte: Inline content block-style](./conformance_prettier_svelte.md#svelte-inline-content-block-style))

### Declaration Tag

- Basic declaration (`{const x = value}` / `{let x = value}`)
- Binding-less `let` (`{let x}` → `{let x;}`)
- Declaration with destructuring
- Declaration in various contexts (root, if, each, snippet, element, component)
- Own line in its fragment, on the same rule as `{@const}` above (`{#snippet}` follows the
  same rule — it declares a binding and hoists alike; see its section below)
- Root siblings around a lifted `<script>` / `<style>` / `<svelte:options>` count as glued —
  the compiler removes the section before its whitespace rules, so the byte gap is not a
  separator (`a<script>…</script>{const y = 2}b` stays welded and renders `ab`)

### Debug Tag

- Empty debug (`{@debug}`)
- Debug with identifiers (`{@debug x, y, z}`)

### Render Tag

- Basic render (`{@render snippet()}`)
- Render with arguments (`{@render snippet(arg)}`)
- Optional render (`{@render snippet?.()}`)
- Dynamic snippet (`{@render children?.()}`)

### Attach Tag

- Basic attach (`{@attach handler}`)
- Attach with arguments (`{@attach tooltip(content)}`)
- Inline attachment function
- Multiple attachments on element
- Attach on component

---

## Snippets

### Basic Snippets

- No parameters (`{#snippet name()}`)
- With parameters (`{#snippet name(a, b)}`)
- With default parameters
- With destructuring
- Parameter comments — interior (`{ a = /* c */ 1 }`), boundary (`a /* c */, b`), dangling (`(/* c */)`)
- Signature head `<TP>(PARAMS)` parsed as a synthetic `function f<TP>(PARAMS) {}`; a parse
  failure rejects the component, matching Svelte's own reader (which hands the same slice to
  `parse_expression_at` as `(PARAMS) => {}` and lets the throw out). A malformed head —
  `fn(a b)`, `fn(,,)`, `fn(1 + )`, `fn(() => 1)`, `fn<T extends>()`, `fn<>()` — is never kept
  as raw text. See `blocks/snippet/{params,ts_generic,ts_generic_constraints}/input_invalid_*`
- Nested snippets
- Recursive snippets
- Own line in its fragment, except when glued to content on both sides — the same rule as the
  declaration tags (see [conformance_prettier_svelte.md §Svelte: Inline content block-style](./conformance_prettier_svelte.md#svelte-inline-content-block-style))

### TypeScript Snippets

- Generic type parameters (`{#snippet name<T>(x: T)}`) — parsed into nodes and routed through
  `tsv_ts`'s type-parameter printer (constraints `<T extends X>`, defaults `<T = X>`, modifiers
  `<const T>`, interior comments `<T /* c */>`, and width-based wrapping of a long generic list,
  which breaks independently of the parameter list)
- Typed parameters (`{#snippet fn(a: string, b: number)}`)
- Typed parameter comments (`{#snippet fn(a: T /* c */, b: U)}`)
- ⚠️ Accepted **without** `lang="ts"` too — Svelte gates every TypeScript reader on the document's
  `ts` flag and rejects, tsv's parser carries no such flag. A tracked over-acceptance across every
  TS-bearing template position, pinned by `script/no_lang_typescript_svelte_prettier_divergence`
  (see [conformance_svelte.md §TypeScript-mode gating](./conformance_svelte.md#typescript-mode-gating-tracked-over-acceptance))

### Snippet Scope

- Lexical scoping
- Access to script variables

### Snippet Props

- Snippet as component prop
- Implicit `children` snippet
- Optional snippet props (with defaults)

---

## Directives

### Legacy on: Directive

- Basic handler (`on:click={handler}`)
- Shorthand (`on:click`)
- With modifiers (`on:click|preventDefault`)
- Multiple modifiers (`on:click|preventDefault|stopPropagation`)
- Multiple events on element

### Event Modifiers

- `preventDefault`
- `stopPropagation`
- `stopImmediatePropagation`
- `passive`
- `nonpassive`
- `once`
- `capture`
- `self`
- `trusted`

### Modern Event Attributes

- Event attribute (`onclick={handler}`)
- Event attribute shorthand (`{onclick}`)
- Passive touch events (`ontouchstart`, `ontouchmove`)

### Bind Directive

**Basic Binding**:

- Expression form (`bind:value={variable}`)
- Shorthand form (`bind:value`)
- `bind:this` (element reference)

**Input Bindings**:

- `bind:value` (text input)
- `bind:checked` (checkbox)
- `bind:group` (radio/checkbox groups)
- `bind:files` (file input)
- `bind:indeterminate` (checkbox)

**Select Bindings**:

- `bind:value` (single select)
- `bind:value` (multiple select)

**Form Reset Support**:

- `defaultValue` attribute (input reverts on form reset)
- `defaultChecked` attribute (checkbox reverts on form reset)
- `<option selected>` (select reverts on form reset)

**Media Bindings (audio/video)**:

- `bind:currentTime`, `bind:playbackRate`
- `bind:paused`, `bind:volume`, `bind:muted`
- `bind:duration`, `bind:buffered`, `bind:seekable` (readonly)
- `bind:seeking`, `bind:ended`, `bind:readyState`, `bind:played` (readonly)

**Video-Specific Bindings**:

- `bind:videoWidth`, `bind:videoHeight` (readonly)

**Image Bindings**:

- `bind:naturalWidth`, `bind:naturalHeight` (readonly)

**Dimension Bindings**:

- `bind:clientWidth`, `bind:clientHeight`
- `bind:offsetWidth`, `bind:offsetHeight`
- `bind:contentRect`, `bind:contentBoxSize`, `bind:borderBoxSize`, `bind:devicePixelContentBoxSize`

**Contenteditable Bindings**:

- `bind:innerHTML`, `bind:innerText`, `bind:textContent`

**Details Element**:

- `bind:open`

**Function Bindings**:

- Get/set form (`bind:value={() => val, (v) => val = v}`)
- Readonly with setter (`bind:clientWidth={null, callback}`)

**Component Bindings**:

- `bind:property` on components
- Two-way binding with `$bindable()` (runtime feature)

### Class Directive

- Expression form (`class:name={condition}`)
- Shorthand form (`class:name`)
- Multiple class directives

**Class Attribute**:

- Object form (`class={{ active: true }}`)
- Array form (`class={[cond && 'name']}`)
- Mixed forms

### Style Directive

- Expression form (`style:property={value}`)
- Shorthand form (`style:property`)
- Important modifier (`style:property|important={value}`)
- Multiple style directives

### Use Directive (Actions)

- Without parameters (`use:action`)
- With parameters (`use:action={params}`)
- Multiple actions on element

### Transition Directives

**Basic Transitions**:

- Bidirectional (`transition:name`)
- In-only (`in:name`)
- Out-only (`out:name`)
- With parameters (`transition:fade={{ duration: 300 }}`)

**Transition Modifiers**:

- Local (`transition:fade|local`)
- Global (`transition:fade|global`)

**Animation Directive**:

- Basic animate (`animate:flip`)
- With parameters (`animate:flip={{duration: 200}}`)

**Transition Events**:

- `onintrostart`, `onintroend`
- `onoutrostart`, `onoutroend`

### Let Directive (Slot Props)

- Basic let (`let:prop={variable}`)
- Let shorthand (`let:prop`)
- Multiple let directives

---

## Special Elements

### svelte:window

- Event binding (`<svelte:window on:keydown={handler} />`)
- Attribute binding (`<svelte:window bind:innerWidth={w} />`)
- `bind:innerWidth`, `bind:innerHeight`
- `bind:outerWidth`, `bind:outerHeight`
- `bind:scrollX`, `bind:scrollY`
- `bind:online`, `bind:devicePixelRatio`

### svelte:document

- Event binding
- `bind:activeElement`, `bind:fullscreenElement`
- `bind:pointerLockElement`, `bind:visibilityState`

### svelte:body

- Event binding (`<svelte:body on:click={handler} />`)

### svelte:head

- Basic usage (`<svelte:head>`)
- Title element
- Meta elements
- Link elements

### svelte:element

- Dynamic element (`<svelte:element this={tag}>`)
- With attributes
- With children
- Void element handling (`this="hr"`)
- Namespace attribute (`xmlns`)
- Repeated `this` — the first binds the tag, later ones stay ordinary attributes

### svelte:component

- Dynamic component (`<svelte:component this={Comp} />`)
- With props
- With children
- Repeated `this` — as above; only the first must be an `{expression}`

### svelte:self

- Recursive component reference

### svelte:fragment

- Non-DOM wrapper
- With slot attribute

### svelte:boundary

- Basic boundary
- `pending` snippet
- `failed` snippet (with error, reset)
- `onerror` handler

### svelte:options

- `runes={true}` / `runes={false}`
- `namespace="svg"` / `namespace="mathml"`
- `customElement` option (string)
- `customElement` option (object)
- `css="injected"`
- Deprecated: `immutable`, `accessors`
- Root-only: it fills `Root`'s options slot rather than becoming a fragment node, so
  there is no nested form and one inside an element or a block is a parse error. Its
  four `root_only_meta_tags` siblings (`<svelte:head>` / `<svelte:window>` /
  `<svelte:body>` / `<svelte:document>`) each have a node type, so a nested one parses
  and the placement rule is left to a later diagnostics pass

### slot

- Default slot (`<slot />`)
- Named slot (`<slot name="x" />`)
- Slot with fallback content
- Slot props

### Reserved `svelte:` namespace

- The ten meta tags above are the whole namespace — any other local name is a parse
  error (`<svelte:foo>`, `<svelte:headx>`, `<svelte:optionsx>`)
- Matched case-sensitively on both halves: `<svelte:Head>` is rejected, `<SVELTE:head>`
  is an ordinary namespaced element
- Only that exact prefix is reserved — `<sveltex:foo>` and `<foo:bar>` stay ordinary
  namespaced elements

---

## Runes (Svelte 5)

### State Runes

**$state**:

- Basic declaration (`let x = $state(value)`)
- In class fields
- Deep reactivity (arrays/objects)

**$state.raw**:

- Non-proxied state (`$state.raw(value)`)

**$state.snapshot**:

- Snapshot of proxy (`$state.snapshot(obj)`)

**$state.eager**:

- Eager updates (`$state.eager(value)`)

### Derived Runes

**$derived**:

- Basic derived (`let y = $derived(expr)`)
- With function body (`$derived.by(() => { ... })`)
- Overriding derived values

### Effect Runes

**$effect**:

- Basic effect (`$effect(() => { ... })`)
- With cleanup function
- Dependency tracking
- Nested effects

**$effect.pre**:

- Pre-update effect (`$effect.pre(() => { ... })`)

**$effect.tracking**:

- Tracking context check (`$effect.tracking()`)

**$effect.pending**:

- Pending promise count (`$effect.pending()`)

**$effect.root**:

- Manual effect scope (`$effect.root(() => { ... })`)

### Props Runes

**$props**:

- Basic props (`let { x, y } = $props()`)
- With defaults
- With rest (`let { a, ...rest } = $props()`)

**$bindable**:

- Bindable prop (`let { x = $bindable() } = $props()`)
- With fallback value

**$props.id**:

- Unique component instance ID (`$props.id()`)
- For attribute linking (`for`, `aria-labelledby`)

### Other Runes

**$inspect**:

- Basic inspect (`$inspect(value)`)
- With custom formatter (`$inspect(x).with(fn)`)
- Trace dependencies (`$inspect.trace()`)

**$host**:

- Custom element host (`$host()`)

---

## Script & Style Sections

### Script Blocks

**Basic Script**:

- Instance script (`<script>`)
- Module script (`<script module>`, and the legacy `<script context="module">`)
- TypeScript script (`<script lang="ts">`)
- Generics (`<script lang="ts" generics="T">`)

**Script Content**:

- TypeScript expressions
- Imports/exports
- Comments
- Escape sequences in strings

### Style Blocks

**Basic Styles**:

- Scoped styles (`<style>`)
- Nested `<style>` elements (inserted as-is, no scoping)

**CSS Scoping**:

- `:global(selector)` modifier
- `:global` block syntax
- Scoped `@keyframes`

**CSS Features (via tsv_css)**:

- All CSS selectors
- All CSS at-rules
- CSS custom properties (`--var`)
- Nesting (CSS nesting syntax)

---

## Comments

### HTML Comments

- Basic comment (`<!-- comment -->`)
- Multi-line comments
- Empty comments
- Comments between elements
- Comments in control flow

### Special Comments

- `svelte-ignore` warnings (`<!-- svelte-ignore a11y_* -->`)
- Multiple ignores (`<!-- svelte-ignore a, b -->`)
- `@component` JSDoc (`<!-- @component -->`)
- `format-ignore` / `prettier-ignore` directive (`<!-- format-ignore -->` emits the next node verbatim — see [directives.md](./directives.md))
- `format-ignore-start` / `-end` range (`<!-- format-ignore-start -->` … `<!-- format-ignore-end -->` preserves a top-level range)

---

# Experimental Async — parse/format supported

Features requiring `experimental: { async: true }` in svelte.config.js. The flag
gates Svelte's *compilation*, not tsv's parse/format — tsv handles all of these
today, each with gating fixtures — and will be removed in Svelte 6 (becomes stable).

## Async Expressions

### await in Script

- Top-level await in `<script>` (`await fetch()`, `await Promise.all()`) — `svelte/script/await_toplevel`
- `await` inside `$derived()` (`let x = $derived(await fn())`) — `svelte/runes/await_derived`

### await in Markup

- Await expression tag (`{await promise}`) — `svelte/expressions/await_markup`
- Await with arithmetic (`{a} + {b} = {await add(a, b)}`) — `svelte/expressions/await_markup`

### Async Utilities

- `fork()` API (`fork(() => { ... })`) — `svelte/runes/async_utilities`
- `fork().commit()` / `fork().discard()` — `svelte/runes/async_utilities`
- `settled()` function (wait for async updates) — `svelte/runes/async_utilities`
- `$effect.pending()` for loading states — see **$effect.pending** under Supported

---

# Out of Scope

These are runtime concerns, not template syntax:

- Store subscriptions (`$storeName`)
- Context API (`setContext`, `getContext`)
- Lifecycle hooks (`onMount`, `onDestroy`)
- Imperative component API (`mount`, `unmount`)
- Reactive built-ins (`Map`, `Set`, `URL`, `Date`)
- Custom element compilation details
- Preprocessor languages (`<style lang="scss">`)
- Automatic class hashing (compilation feature)

---

# Compatibility

Parse output matches Svelte's parser and formatting matches Prettier, except for the intentional divergences cataloged in [conformance_svelte.md](./conformance_svelte.md) and [conformance_prettier.md](./conformance_prettier.md).

## Intentional Differences

**Self-closing non-void elements**: tsv expands `<div />` to `<div></div>` per Svelte's warning. Prettier keeps the self-closing form.
