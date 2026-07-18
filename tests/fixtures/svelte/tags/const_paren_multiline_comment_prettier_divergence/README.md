# const_paren_multiline_comment_prettier_divergence

A **multi-line block comment leading a parenthesized `{@const}` value**
(`{@const x = /* c⏎*/ (a > b)}`). The grouping parens are redundant, so both formatters
strip them.

Unlike the other `{…}` paren-comment cases (tags, directives, block heads), **prettier
does *not* drop the comment here** — the `{@const}` assignment keeps a leading value
comment. So the only divergence is **block layout**: tsv lays the enclosing block
(`{#if}` / `{#snippet}` / …) out block-style whenever its body renders multiline (head
and tail intact, body on its own indented line), where prettier keeps the block tight.
That is a pre-existing divergence, independent of the comment.

tsv (the `input.svelte` fixed point) — which prettier also keeps stable when handed it,
so `input.svelte` is dual-stable:

```svelte
{#if cond}
	{@const x =
		/* c
		 */ a > b}
{/if}
```

`prettier_variant_tight_block.svelte` pins prettier's own stable form when it reflows the
compact authoring — the same comment, but the block kept tight
(`{#if cond}{@const x =⏎\t\t/* c⏎\t\t */ a > b}{/if}`); tsv normalizes that back to the
block-style `input.svelte`. `unformatted_ours_paren.svelte` (the paren authoring) and
`unformatted_ours_bare.svelte` (the bare authoring) both normalize to `input.svelte`
under tsv; prettier keeps the block tight for both.

The bug this fixture guards is the paren-form **idempotency**: before the fix,
`unformatted_ours_paren.svelte` stayed inline on the first pass (the stripped paren left
the comment positional and the value gap emitted it without forcing the break) and only
reindented on a second pass. It now reaches `input.svelte` on the first pass, matching the
bare authoring.

## Reason

The comment is preserved and stable on the first pass; the block-style layout is tsv's
standing choice for a block whose body renders multiline. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
