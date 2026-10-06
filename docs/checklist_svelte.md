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
- Special elements inside `<pre>` (`<svelte:element>`, `<svelte:component>`, `<svelte:self>`, `<svelte:boundary>`, `<svelte:fragment>`, `<slot>`) — content printed verbatim, laid out inline like a component (Svelte carries the ancestor's whitespace-preserving state into their fragments)
- Text node normalization
- Leading/trailing whitespace handling

---

## Attributes

### Basic Attributes

- Standard attributes (`name="value"`)
- Empty string values (`attr=""`)
- Boolean attributes (`disabled`, `checked`)
- `class` whitespace on HTML elements (prettier-plugin-svelte's two regexes): runs collapse to one space, trailing whitespace per line and at the value's end is dropped, leading whitespace and newlines stay — and the one separator space before a following `{expr}` is keyed on the text's LAST line, so an `{expr}` on its own indented continuation line keeps that indentation (`svelte/attributes/class_whitespace`, `svelte/attributes/class_multiline_expression`). Components are never normalized; `<svelte:element>` is normalized on purpose where prettier leaves it verbatim (a cataloged divergence)
- Names with non-identifier chars (`a%b`, directive `on:click%x`) — read up to `[\s=/>"']`, mirroring Svelte's `read_tag`. ⚠️ The run is measured from the name's **first byte**, never resumed from the lexer token's end: the marker braces are one token *across* the gap (`{ #` tokenizes like `{#`, mirroring Svelte's `tag()`), so in a top-level `<script>`/`<style>` head — the one place a `{` is an ordinary name character — resuming there folded the author's whitespace into the name (`<script { #a}>` read `{ #a}` where Svelte reads `{` then `#a}`) and stepped over the `/` in `{/a}`, a terminator Svelte stops at, so a head canonical rejects parsed. Pinned by `svelte/script/brace_attribute_literal/`
- `<!--` inside a tag is a name, not a comment: Svelte's attribute reader has no HTML comment, so `<input <!-- c -- />` carries the boolean attributes `<!--`, `c` and `--`, and no `-->` is looked for or owed. See `svelte/attributes/name_comment_opener/`

### Dynamic Attributes

- Expression attributes (`name={expr}`)
- Mixed text+expression (`"text{expr}text"`)
- Shorthand attributes (`{variable}`) — the interior goes through the same `read_identifier` as the block-head and block-binding positions, so all of its rules apply: the ECMAScript `ID_Start`/`ID_Continue` classes (`{℘}` valid, `{a²}` not), an empty name (`{123}`, `{1a}`, `{²}`), and a reserved word (`{this}`, `{class}`). See `attributes/{shorthand_numeric_invalid,shorthand_reserved_invalid,shorthand_unicode_identifier}` and, for Svelte's whole reserved-word list across every `read_identifier` position, `tests/svelte_read_identifier.rs`
- Spread attributes (`{...object}`)
- Multiple spread attributes
- Every `{`-led attribute reaches the `{@attach}` / `{...spread}` / `{shorthand}` split, the block markers included. Svelte has no marker token at this position — `read_attribute` eats the brace and runs `read_identifier`, so `{#`, `{:` and `{/` merely leave a non-identifier interior and the shorthand reader rejects it. tsv's lexer classifies those braces first, so routing on `LeftBrace` alone dropped them into the attribute-**name** run: `<div {#if a}>` came back as a `RegularElement` carrying two boolean attributes named `{#if` and `a}` — fabrication, not just an over-acceptance. See `attributes/shorthand_block_marker_invalid/`
- A `{#…}` block or `{@…}` tag is rejected in an attribute value — one instance of the *sequence* rule stated under [§Expression Tags](#expression-tags), which the value readers share with `<textarea>` content. See `attributes/value_block_tag_placement/`

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
- A `{#…}` block or `{@…}` tag is rejected where only a *sequence* — a run of text and `{expr}` chunks — belongs, with Svelte's own wording (`{#if ...} block cannot be in attribute value`, `{@html ...} tag cannot be inside <textarea>`). Svelte asks this once, in `read_sequence`, *before* the expression is read; tsv reaches the same sequences by five routes — `<textarea>` RCDATA content, a quoted attribute value, an unquoted one, and the two directive arms that take their `{…}` off the token stream — so one guard is called from all five (`SvelteParser::check_sequence_placement`). ⚠️ The marker need not be **glued** to the `{` — the guard skips the gap with Svelte's own `allow_whitespace()` class. This is the one place tsv is deliberately wider than `read_sequence`, which asks `parser.match('#')` right after `eat('{')` and so hands `{ #if}` to acorn: both parsers still reject, only the wording differs. The glued-only reading left three things broken — `{ @html x}` came back `Expected 'class' after 'decorator'`, `a="{ #if c}a{/if}"` died as `Unterminated string literal` when the `{/if}`'s `/` opened a regex that never closes, and `{ #x in y}` *parsed*, whereupon the printer normalized the brace to `{#x in y}` and the guard rejected tsv's own output. A byte read at a fixed offset from the brace assumes a gap of width zero, which is precisely the gap the printer closes. Template position is untouched: there `{ #each}` opens a real block, since Svelte's `tag()` does run `allow_whitespace()` (`blocks/open_whitespace/`). Without the guard the brace contents reached the TypeScript expression parser and came back answering another language's question — and `{#x in y}`, the one production where a private name is an operand, *parsed*. See `svelte/elements/textarea_block_tag_placement/` and `svelte/attributes/value_block_tag_placement/`, with the wording pinned by `tests/svelte_sequence_block_tag_placement.rs` and the guard's outer edge by `svelte/script/static_attribute_block_tag_literal/` — a top-level `<script>`/`<style>` head runs `read_static_attribute`, which reads no sequence at all, so the same spellings are literal text there

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
- A block's alternate is filled once — a second `{:else}`, or an `{:else if}` following an `{:else}`, is rejected, as Svelte's reader rejects it. See [conformance_svelte.md](./conformance_svelte.md) §Block Continuation Clauses
- Nested if blocks
- If with expressions only
- If with mixed content

### Each Blocks

- Basic each (`{#each items as item}`)
- With index (`{#each items as item, i}`)
- With key (`{#each items as item (item.id)}`)
- With index and key (`{#each items as item, i (key)}`)
- Each else (`{:else}`) — filled once, like an if block's alternate: a second `{:else}` is rejected, as Svelte's reader rejects it. See [conformance_svelte.md](./conformance_svelte.md) §Block Continuation Clauses
- Destructuring - object (`{#each items as { a, b }}`) — spaced braces match prettier; the lone divergence is the empty pattern (`{}`), see [conformance_prettier_svelte.md](./conformance_prettier_svelte.md)
- Destructuring - array (`{#each items as [a, b]}`)
- Destructuring with rest (`{#each items as {a, ...rest}}`)
- Destructuring with defaults (`{#each items as {a = 1}}`) — prettier divergences: literal defaults normalize (single quotes + numeric form), and a renamed property keeps its key where prettier drops it. See [conformance_prettier_svelte.md](./conformance_prettier_svelte.md)
- Typed context binding (`{#each items as item: number}`, lang="ts")
- Typed destructured context binding (`{#each items as { a }: { a: number }}`, `{#each pairs as [n]: [number]}`) — the annotation attaches to the pattern; the wire `end` widens past it, matching Svelte's `read_pattern` (`loc.end` follows the widened `end`, where Svelte's stays at the bracket). See [conformance_svelte.md](./conformance_svelte.md)
- Each without `as` (`{#each items}`, `{#each items, i}`, `{#each items, i (key)}`) — index/key are valid without a context binding; all route through the same index/key parser as the `as` form
- Each whose head holds an `as` but no binding (`{#each items as A satisfies B}`, lang="ts") — Svelte unwinds a head assertion only when the expression's **outermost** node is a `TSAsExpression`, so a run ending on `satisfies` keeps the whole run as the iterable and the block is binding-less. A later `satisfies` cancels an earlier `as` the same way (`{#each items as A[] as item satisfies B}` — `item` is a type). Routes through the no-`as` index/key tail, so the two spellings of "no binding" produce one shape. See `blocks/each/type_assertion_satisfies_no_binding/`
- An `as` nested inside the head expression is TypeScript's, not the separator (lang="ts") — the separator is read only at the head's own level, so a parameter list (`{#each (x = y as A) => {} as item}`) and a function or class body (`{#each function () { return y as A; } as item}`, a class field's computed key and initializer, a static block) keep their assertion. See `blocks/each/type_assertion_arrow_param_default/`, `blocks/each/type_assertion_body/`
- A head ending on a parenthesized operand (`{#each a || (b as A) as item}`, lang="ts") ends at its `)`, where Svelte's unwind stops one character early and prettier-plugin-svelte then drops the paren — a cataloged divergence. See `blocks/each/ts_head_paren_tail_svelte_prettier_divergence/`
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
- Destructuring in `then`/`catch` bindings (`{:then {a = 1}}`) — same empty-pattern + default-value divergences as each blocks
- Typed `then`/`catch` value (`{:then value: number}`, `{:catch error: Error}`, lang="ts") — including a destructured pattern (`{:then { a }: { a: number }}`), whose wire `end` widens past the annotation as a typed each binding's does
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
- Where an island ends: every `{…}` is closed by a byte scan ahead of the parse, which must read each `/` as the parser will — a division after a type-argument list's `>` (`{f<T> / 2}`, answered by `tsv_ts::closes_type_arguments`, the parser's own type-argument predicates asked from the `>` end), a postfix `!` (a `!` run glued to what precedes it), a numeric literal's trailing `.`, a non-ASCII identifier character or a regex literal; a regex after a comparison, an arrow's `=>`, a prefix `!` (one with whitespace or a line break before it — a line comment, or a block comment spanning lines: `if (c) !/re/`, `a // c⏎!/re/`, `a /* c⏎*/!/re/` — or glued to a statement header's `)`: `if (c)!/re/`, answered by `tsv_ts::closes_statement_header`), a spread, or a block or tag head's keyword (`{#if /re/.test(s)}` — the head scan starts past the keyword). An `{#each}` key's `)` is found by the same scan's paren twin, so a regex holding a `)` stays inside it (`svelte/blocks/each/key_paren_scan/`). A `{#snippet}` parameter list's `)` is found by Svelte's own raw paren count instead, which Svelte then parses the slice of: a quote or `//` in a regex default ends nothing, and a `(` or `)` in any string, comment or regex default is counted as Svelte counts it, so an unbalanced one (`a = ')'`) is rejected as Svelte rejects it (`svelte/blocks/snippet/param_default_regex/`). A misread `/` is bounded to its line, since a regex literal cannot hold a line terminator (`svelte/expressions/division_after_*/`). A `}` is read as acorn's tokenizer reads it (`tsv_ts::ACORN_ISLAND_GRAMMAR`, a forward walk keeping acorn's token-context stack): an object literal's ends an operand (`{ {} / 2}`), as does a function or class expression's body when acorn takes the keyword for an expression's (`{x + function () {} / 2}`, a named `{async function fn() {} / 2}`); a block's does not, nor does a function or class body acorn reads as a statement's — among them the island's first token, an anonymous `async function`, and a conditional's alternate outside any `(…)`, object literal or `${…}` (`{c ? x : function () {} / 2}` and `{[c ? x : function () {} / 2]}`, which Svelte rejects) (`svelte/expressions/division_after_object_literal/`, `svelte/expressions/division_after_function_parens_prettier_divergence/`). Inside a nested body — a function's, an arrow's, a class's — a statement may begin, where acorn's parser re-reads the tokenizer's division as a regex, so a brace directly in one answers as a block's (`svelte/expressions/division_after_block/`); an object literal divided there is over-rejected on one line (`{() => { x = {} / 2 }}`), and a `(…)` or `${…}` there is read in full. A postfix `!` glued to the `}` leaves the answer the brace's, a function or class body ending an operand before it (`svelte/expressions/division_after_non_null/`). A spread's scan starts past its `...`, which Svelte reads before handing acorn the expression.
- A regex-led directive value (`style:color={/a/.source}`, `class:`, `on:`, `use:`, `transition:`, `animate:`): the lexer tokenizes `{/` as a block close before it knows where it stands, and a directive value takes it as an expression, as Svelte's `read_sequence` does (`svelte/directives/regex_value/`)
- A text-position tag whose expression PRINTS first a regex literal takes a pair (`{(/a/.test(s))}`), since Svelte reads `{/` there as a block close — a cataloged prettier divergence (`svelte/expressions/regex_led_tag_prettier_divergence/`)

### HTML Tag

- Basic html (`{@html expr}`)
- HTML with long content

### Const Tag

- Basic const (`{@const x = value}`)
- Const with destructuring
- Const in various contexts (if, each, await)
- A type annotation whose TS syntax the head's own splitter must read through — a `,`
  inside type arguments and the `=>` of a function type, on either side of the `=`
- The head is printed verbatim by prettier once the binding is annotated (see
  [conformance_prettier_svelte.md §Svelte: annotated `{@const}` head verbatim](./conformance_prettier_svelte.md#svelte-annotated-const-head-verbatim))
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
- Root-only, with its four `root_only_meta_tags` siblings (`<svelte:head>` /
  `<svelte:window>` / `<svelte:body>` / `<svelte:document>`): each is legal only as a
  direct child of the component root, and at most once per component. One nested in an
  element, a component, a block or another meta tag, or a second one anywhere, is a parse
  error, as in Svelte (`svelte_meta_duplicate` is raised ahead of
  `svelte_meta_invalid_placement`). `<svelte:options>` additionally fills `Root`'s options
  slot rather than becoming a fragment node, so it has no nested form at all. See
  `svelte/special_elements/svelte_options_root_only/` and the `input_invalid_*` cases
  beside each sibling's fixture

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

**Placement**:

- Canonical section order (`<svelte:options>`, module script, instance script, template, `<style>`), each section with the comments that travel with it
- A section written **between** two template nodes: the neighbours join as if the section and its travelling comments were absent — no whitespace keeps them glued, any whitespace is a space, any newline a line break, and a blank line only when it sits before or after the run (a blank inside the run travels with it)

**Nested in markup** (`<script>` / `<style>` inside an element or a block):

- Body formatted as the top-level section's is, one indent level inside its tags
- Renders no box — keeps its glue where a line break beside it would render, and takes its own line only where a break is render-free (merging into whitespace that already renders, or at a line-box edge: a block element or a block parent's content edge)
- Inside `<pre>`: body kept verbatim, and the closing tag never splits (Svelte ends a nested raw-text body at the first literal `</script>` / `</style>`), so a too-wide line breaks at the opening tag's `>` instead when the body opens on a visible byte

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
- `<!--` in raw content is content, not a comment: a nested `<script>` / `<style>` body and `<textarea>` content are read raw, so `<textarea><!-- x</textarea>` holds the text `<!-- x` and no `-->` is looked for or owed — where an element's content is template (`<title>`, `<pre>`) the same bytes are an unterminated comment. See `svelte/elements/textarea_comment_opener/`, `svelte/elements/nested_script_style_comment_opener_prettier_divergence/`

### Special Comments

- `svelte-ignore` warnings (`<!-- svelte-ignore a11y_* -->`)
- Multiple ignores (`<!-- svelte-ignore a, b -->`)
- `@component` JSDoc (`<!-- @component -->`)
- `format-ignore` / `prettier-ignore` directive (`<!-- format-ignore -->` emits the next node verbatim — see [directives.md](./directives.md))
- `format-ignore-start` / `-end` range (`<!-- format-ignore-start -->` … `<!-- format-ignore-end -->` preserves a top-level range)
- Directive directly above a `<script>` / `<style>` freezes that section alone, wherever the canonical section order prints it; above `<svelte:options>` it freezes nothing
- Editor region markers around a hoisted section (`<!-- #region … -->` above a `<script>` / `<style>` / `<svelte:options>`, `<!-- #endregion -->` directly below it): the `#endregion` travels below its section through the canonical reorder, the author's blank line between them kept — prettier-plugin-svelte's region-end trail, precedence included (a following `<script>`/`<style>` still claims the marker as its leading comment; a following `<svelte:options>` does not)

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
