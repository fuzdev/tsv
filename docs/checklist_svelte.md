# Svelte Language Support

Comprehensive reference for Svelte template syntax features supported by tsv's parser and formatter.

## Coverage

All Svelte 5.x template syntax features are supported, as enumerated below; parse conformance is measured against Svelte's parser on the fixture suite and corpus (see [conformance_svelte.md](./conformance_svelte.md)). Experimental features that require compiler flags are listed under [Future Work](#future-work).

**Spec References**:

- Svelte docs: `../../svelte/documentation/docs/`
- Compiler source: `../../svelte/packages/svelte/src/`
- Existing fixtures: `tests/fixtures/svelte/` (500+ fixtures)

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
- Shorthand attributes (`{variable}`)
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
- Nested if blocks
- If with expressions only
- If with mixed content

### Each Blocks

- Basic each (`{#each items as item}`)
- With index (`{#each items as item, i}`)
- With key (`{#each items as item (item.id)}`)
- With index and key (`{#each items as item, i (key)}`)
- Each else (`{:else}`)
- Destructuring - object (`{#each items as { a, b }}`) — spaced braces match prettier; the lone divergence is the empty pattern (`{}`), see [conformance_prettier.md](./conformance_prettier.md)
- Destructuring - array (`{#each items as [a, b]}`)
- Destructuring with rest (`{#each items as {a, ...rest}}`)
- Destructuring with defaults (`{#each items as {a = 1}}`) — prettier divergences: literal defaults normalize (single quotes + numeric form), and a renamed property keeps its key where prettier drops it. See [conformance_prettier.md](./conformance_prettier.md)
- Each without `as` (`{#each items}`, `{#each items, i}`, `{#each items, i (key)}`) — index/key are valid without a context binding; all route through the same index/key parser as the `as` form
- Nested each blocks
- Binding ends at `}` — a stray comment, leftover index/key fragment, or junk after the binding is rejected (matching Svelte's final `eat('}')`), never silently dropped. Index must be a bare identifier; the key `(…)` is matched with the trivia-aware bracket scanner. See `blocks/each/{no_as_with_index_key, with_index_key/input_invalid_*}`

### Await Blocks

- Basic await (`{#await promise}...{/await}`)
- Pending content
- Then clause (`{:then value}`)
- Catch clause (`{:catch error}`)
- Shorthand then (`{#await promise then value}`)
- Shorthand catch (`{#await promise catch error}`)
- Destructuring in `then`/`catch` bindings (`{:then {a = 1}}`) — same brace-hugging + default-value divergences as each blocks
- Typed `then`/`catch` value (`{:then value: number}`, lang="ts")
- `then`/`catch` value is a bare pattern — a comment immediately before it, or between it and the `:`/`}`, is rejected (matching Svelte's `read_pattern`), never relocated or dropped; a comment *inside* a destructure (`{a /* c */}`) or *inside* the type (`value: /* c */ number`) stays valid. See `await/{then_shorthand,then,catch_shorthand,catch}/input_invalid_*_comment`
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

### Declaration Tag

- Basic declaration (`{const x = value}` / `{let x = value}`)
- Binding-less `let` (`{let x}` → `{let x;}`)
- Declaration with destructuring
- Declaration in various contexts (root, if, each, snippet, element, component)

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
- Nested snippets
- Recursive snippets

### TypeScript Snippets

- Generic type parameters (`{#snippet name<T>(x: T)}`) — parsed into nodes and routed through
  `tsv_ts`'s type-parameter printer (constraints `<T extends X>`, defaults `<T = X>`, modifiers
  `<const T>`, interior comments `<T /* c */>`, and width-based wrapping of a long generic list,
  which breaks independently of the parameter list)
- Typed parameters (`{#snippet fn(a: string, b: number)}`)
- Typed parameter comments (`{#snippet fn(a: T /* c */, b: U)}`)

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

### svelte:component

- Dynamic component (`<svelte:component this={Comp} />`)
- With props
- With children

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

### slot

- Default slot (`<slot />`)
- Named slot (`<slot name="x" />`)
- Slot with fallback content
- Slot props

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

# Future Work

Experimental features requiring `experimental: { async: true }` in svelte.config.js. The flag will be removed in Svelte 6 (becomes stable).

## Async Expressions

### await in Script

- Top-level await in `<script>` (`await fetch()`, `await Promise.all()`)
- `await` inside `$derived()` (`let x = $derived(await fn())`)

### await in Markup

- Await expression tag (`{await promise}`)
- Await with arithmetic (`{a} + {b} = {await add(a, b)}`)

### Async Utilities

- `fork()` API (`fork(() => { ... })`)
- `fork().commit()` / `fork().discard()`
- `settled()` function (wait for async updates)
- `$effect.pending()` for loading states

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
