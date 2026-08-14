# `{:else if}` after `{:else}` — Svelte Divergence

A plain `{:else}` leaves the `IfBlock` on the parser's stack, so the next `{:else if}`
re-enters the same `next` arm (`1-parse/state/tag.js`) and runs the same unguarded
`block.alternate = create_fragment()` before appending its nested `IfBlock`. The
first alternate is discarded with everything in it:
`{#if cond}text1{:else}text2{:else if other}text3{/if}` parses to an AST holding
`text1` and `text3` — **`text2` is deleted**.

**tsv rejects instead** — `{:else if} cannot follow {:else}`, not the sibling's
"duplicate" wording, because nothing here is written twice: this is the block's
*first* `{:else if}`, landing on an alternate the `{:else}` already took. Naming the
pair says which two clauses collide; naming one of them as a duplicate would report a
clause the author never repeated.

The sibling [if/else_duplicate](../else_duplicate_svelte_divergence/) carries the full
argument for the divergence; this fixture is the `{:else if}` spelling of it, held
separately because one input rejects with one message and pins one canonical AST.

See [conformance_svelte.md §Block Continuation Corrections](../../../../../../docs/conformance_svelte.md#block-continuation-corrections).
