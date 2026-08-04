# head_prettier_ignore_prettier_divergence

An own-line directive in a **block head's** gap — `{#if`/`{:else if`→test, `{#each`→collection,
`{#key`→value, `{#await`→promise, and an `{#each}` key's own `(`→value — freezes that whole head
expression. The block keyword, the `as` clause, the key's parens and the closing `}` are all
parent-owned and stay outside the slice:

```svelte
{#if
	// prettier-ignore
	aaa  +  bbb
}
	text1
{/if}
```

Prettier **relocates** the directive flush onto the keyword's line (`{#if // prettier-ignore`)
and freezes anyway. tsv keeps the line the author gave it: a head-trailing directive is inert
under the placement floor, so following the relocation would lose the freeze on tsv's own second
pass. The geometry is the block head's existing broken form — the head expression indented on
its own line, the `}` dangling below — which tsv already takes whenever a leading line comment
breaks the head, and which prettier already diverges from independently (see
[Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)).

A sibling block the freeze does not reach still normalizes.

## Reason

Rule A binds a directive to the node that follows it; ◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
