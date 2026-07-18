# head_paren_multiline_comment_prettier_divergence

A **multi-line block comment leading a parenthesized block-head expression** — the
`{#if}` / `{#each}` / `{#key}` / `{#await}` condition (`{#if /* c⏎*/ (a > b)}`). The
grouping parens are redundant, so both formatters strip them — but tsv keeps the comment
and reaches its stable, idempotent form, while prettier **drops the comment** as it
strips the parens.

tsv (the `input.svelte` fixed point):

```svelte
{#if /* c
 */ a > b
}
	x
{/if}
```

Prettier: `{#if a > b}⏎\tx⏎{/if}` — the leading comment is lost with the parens.

`unformatted_ours_paren.svelte` is the compact paren authoring tsv normalizes to
`input.svelte`; prettier drops the comment, so it carries the divergence.
`variant_comment_dropped.svelte` pins prettier's comment-gone endpoint (fully tight),
which is dual-stable — so once prettier has run the comment is unrecoverable; preserving
it up front is the only lossless path. `output_prettier.svelte` pins prettier's own form
of `input.svelte`: it keeps the comment but lays the head tight (`a > b}`, no `}` dangle)
— the pre-existing block-layout divergence, independent of the comment (see below).

Two independent divergences meet in this fixture:

- **Comment preservation on the paren path** (this bug) — prettier drops a leading
  multi-line block comment when it strips redundant parens; tsv keeps it. See
  [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
- **Block-style layout** (pre-existing) — a block whose head/body renders multiline lays
  out block-style in tsv (head and tail intact, body indented), where prettier lets the
  boundary decide. See
  [conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Reason

User comments are valuable and shouldn't be silently removed; the comment is
syntactically valid here, and reproducing prettier's paren-stripped form would drop it.
tsv preserves the comment and reaches its stable form on the first pass. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
