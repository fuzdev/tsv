# newline_pending_prettier_divergence

An `{#await}` whose pending branch is only a line break — the author broke the block open at the
pending section and wrote nothing there. The branch renders nothing (the compiler trims a
whitespace-only fragment to nothing), so folding it into the `then` / `catch` head shorthand would
be render-safe, and prettier folds it: `{#await promise then value}`. tsv keeps the full form
instead, the pending branch as an empty line of its own in the block form.

A newline-authored section boundary is the one whitespace signal tsv keeps in a block: it holds the
construct multiline, where a space-only boundary is render-free and trims away. A space-only pending
therefore folds exactly as prettier folds it
([whitespace_pending_multiline](../whitespace_pending_multiline/)), and the newline-authored one keeps
the section the author opened.

- `input.svelte` — tsv's form: a then section, a catch section, both, and inside an element; then a
  pending-only block (`{#await promise}⏎⏎{/await}`, the form an empty `{#if}` / `{#each}` /
  `{#key}` body takes), bare and inside a `<span>`.
- `unformatted_ours_single_newline.svelte` — each pending branch a single newline and nothing
  indented; tsv normalizes it to `input.svelte`.
- `unformatted_ours_dropped_then.svelte` — the last two blocks written with a newline-authored
  `{:then}` that renders nothing (`{#await promise then value}⏎{/await}`, and the full-form
  `{#await promise}{:then value}⏎{/await}` in the `<span>`). The section drops, and the newline the
  author wrote keeps the block form, so tsv prints the pending-only block.
- `output_prettier.svelte` — prettier folds every pending branch into the head shorthand, and prints
  the pending-only block as `{#await promise}{/await}`.

## Reason

Design choice: a newline-authored boundary keeps its meaning, and dropping the empty section the
author opened is a layout decision the render does not force. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).
