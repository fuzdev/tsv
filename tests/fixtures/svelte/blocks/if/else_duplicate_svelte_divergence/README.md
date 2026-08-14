# Duplicated `{:else}` clause — Svelte Divergence

Svelte's block-continuation reader (`next`, `1-parse/state/tag.js`) handles
`{:else}` by **replacing** the block's alternate outright:

```js
block.alternate = create_fragment();
```

There is no duplicate guard, so a second `{:else}` discards the first alternate
fragment and everything in it. `{#if cond}text1{:else}text2{:else}text3{/if}`
parses, and the resulting AST holds `text1` and `text3` — **`text2` is deleted**.
Prettier, printing from that AST, emits the same loss.

**tsv rejects instead** (`Duplicate {:else} clause`). Reproducing Svelte here means
a formatter that silently deletes a branch of the author's markup, which is the
robustness bar's first item; there is no reading of the input under which those
bytes survive, so rejecting is the only lossless answer. Svelte itself takes that
view one block over — `{#await}`'s reader raises `block_duplicate_clause` for a
second `{:then}` or `{:catch}` — so this is an inconsistency in the canonical
reader rather than a designed behavior, and tsv applies the `{#await}` rule to all
three continuations.

Because the canonical parser accepts the input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject). The
`tsv_rejects.txt` marker pins tsv's rejection while `expected_svelte.json` pins the
canonical AST — including its missing branch, so the argument for the divergence
dies loudly if Svelte ever adds the guard.

The other two spellings that reach the same unguarded assignment are their own
fixtures, each pinning its own error text and its own truncated canonical AST:
[if/elseif_after_else](../elseif_after_else_svelte_divergence/) (an `{:else if}`
following an `{:else}`) and
[each/else_duplicate](../../each/else_duplicate_svelte_divergence/) (`{#each}`'s
`block.fallback`).

See [conformance_svelte.md §Block Continuation Corrections](../../../../../../docs/conformance_svelte.md#block-continuation-corrections).
