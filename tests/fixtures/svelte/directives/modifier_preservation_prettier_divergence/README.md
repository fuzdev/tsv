# modifier_preservation_prettier_divergence

Directive types **without official modifier support** — `use:`, `bind:`, `class:`,
`animate:`, `let:` — carrying an unofficial `|modifier`. tsv preserves the modifier
text verbatim; prettier silently drops it.

tsv (idempotent):

```svelte
<div use:action|mod1|mod2></div>
<input bind:value|mod />
<div class:class1|mod></div>
```

Prettier drops every `|mod`:

```svelte
<div use:action></div>
<input bind:value />
<div class:class1></div>
```

Svelte's parser is permissive: it splits on `|` for *every* directive and records the
trailing names in `modifiers`, even for the five types whose published `.d.ts` declares
no `modifiers` field. tsv matches Svelte's parser AST exactly — `expected.json` carries
`['mod1', 'mod2']`, `['mod']`, … so this is **not** a `_svelte_divergence`. prettier-plugin-svelte,
by contrast, only re-emits modifiers for the three officially-supporting types
(`on:` / `transition:` / `style:`) and drops the text for these five — silently deleting
source the user wrote.

The officially-supporting types preserve their modifiers in **both** formatters, so they
are not a divergence — see the per-type `on/`, `transition/`, `style/` fixtures.

## Reason

A formatter must not silently delete source the user wrote — a `|modifier` is
semantics-bearing text, not whitespace. Prettier dropping it is content loss; tsv
preserves it (and matches Svelte's parser AST while doing so). See
[conformance_prettier.md §Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [bind/function_comment_inline_block](../bind/function_comment_inline_block_prettier_divergence/) — prettier drops a leading comment in a `bind:` sequence; tsv preserves
- [on/long](../on/long_prettier_divergence/) — officially-supported `on:` modifiers (preserved by both formatters)
