# Duplicated `{:else}` clause — Svelte Divergence

`{#each}`'s arm of the block-continuation reader (`next`, `1-parse/state/tag.js`)
writes its fallback the same unguarded way `{#if}` writes its alternate:

```js
block.fallback = create_fragment();
```

So a second `{:else}` discards the first fallback and everything in it.
`{#each items as item}text1{:else}text2{:else}text3{/each}` parses to an AST
holding `text1` and `text3` — **`text2` is deleted**.

**tsv rejects instead** (`Duplicate {:else} clause`). The sibling
[if/else_duplicate](../../if/else_duplicate_svelte_divergence/) carries the full
argument for the divergence; this fixture is the `{#each}` half of it, held
separately because the two block types reach the loss through different fields and
pin different canonical ASTs.

The `{:else if}` spelling is not a divergence here — canonical's `{#each}` arm eats
`else` and then requires `}`, so `{#each … }{:else if b}` is rejected by both
parsers.

See [conformance_svelte.md §Block Continuation Corrections](../../../../../../docs/conformance_svelte.md#block-continuation-corrections).
