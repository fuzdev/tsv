# multiline_block_body_prettier_divergence

A frozen node whose bytes span lines, inside a block body — `{#if}`, `{#each}`, an `{:else}`
branch, `{#snippet}`, an `{#await}` phase, `{#key}`. The line break in the frozen bytes breaks the
body holding it — as prettier's `literalline` breaks every group around a frozen template node —
so tsv lays the block out block-style: head and tail intact, the body on its own indented lines,
the frozen bytes as written. Hug is all-or-nothing across the construct, so the `{#if}` branch
beside the frozen `{:else}` branch takes its own line too, and so does the `{:then}` phase beside
the frozen pending phase. A freeze whose bytes fit on one line, and the same element unfrozen, are
the controls: neither spans lines, so both stay hugged.

- `prettier_variant_hug.svelte` — every body glued to its head and tail: prettier keeps it
  hugged, tsv normalizes it to `input.svelte`.
- `unformatted_spaces.svelte` — spaces at the body boundaries: both formatters normalize it to
  `input.svelte`.

All of them render identically.

## Reason

Design choice and convergence, render-free under Svelte 5. A block whose body renders over
several lines lays out block-style however the author wrote the body's boundary, which the
compiler trims; a frozen node that spans lines is such a body, as is the unfrozen
`{#if cond}text1<b>{#if cond2}inline{/if}</b>text2{/if}`. Prettier keeps a body glued to its head
and tail hugged, holding one stable form per authoring.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive),
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
(its §Scope reaches block bodies),
and [conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).

## Related

- [content_boundary_convergence](../../../blocks/content_boundary_convergence_prettier_divergence/)
  — the block-body rule for unfrozen content
- [multiline_inline_parent_long](../multiline_inline_parent_long_prettier_divergence/) — the same
  freeze in an inline element
