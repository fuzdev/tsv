# dropped_then_head_long_prettier_divergence

An `{#await}` whose only section is a `{:then}` that renders nothing — here a space-only body. The
section drops, marker and binding, so the block prints exactly as the section-less
`{#await expr}{/await}` does: `{/await}` hugged to the head at the 100/101 boundary alike, the
head's arguments wrapping once it passes print width and the close staying on the `)` line.

- `input.svelte` — tsv's form: the head at exactly 100 columns, then at 101.
- `unformatted_ours_then_shorthand.svelte` — the section written as the head shorthand
  (`{#await fn(…) then value} {/await}`); tsv normalizes it to `input.svelte`.
- `unformatted_ours_then_section.svelte` — the section written in full
  (`{#await fn(…)} {:then value} {/await}`); tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier drops the section too, but never wraps the block head, so the
  101-column row stays on one line past print width.

A dropped section decides nothing about the block's layout beyond what its authored boundary says:
a space-only one is render-free, so the block stays hugged, while a newline-authored one keeps the
block form ([newline_pending](../newline_pending_prettier_divergence/)).

## Reason

Print width: tsv wraps a block head past 100 columns where prettier never does. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).
