# empty_catch_head_long_prettier_divergence

An `{#await}` whose only section is an **empty `{:catch}`** — kept, since it handles a rejection
([empty_catch_no_binding](../empty_catch_no_binding_prettier_divergence/)), but printing no body.
The section's marker rides in the head as the `catch` shorthand, so what follows the head is only
`{/await}`, and the block lays out as the section-less `{#await expr}{/await}` does: `{/await}`
hugged to the head at the 100/101 boundary alike, the head's arguments wrapping once it passes
print width and the close staying on the `)` line — with or without a binding.

- `input.svelte` — tsv's form: the head at exactly 100 columns, then at 101, for `catch` and
  `catch error`; an identifier head at the same boundary, which has no arguments to wrap and so
  breaks before its `catch` clause; a newline-authored body, which keeps the block form; and a
  catch pattern a line comment forces open inside a `{#if}` and an inline `<span>`, whose forced
  break reaches both parents, which go multiline while the block itself keeps `{/await}` hugged.
- `unformatted_ours_glued.svelte` — every hugged block authored on one line, the forced-open
  pattern's block hugged into its parents; tsv wraps the 101-column heads. The newline-authored
  body is a single newline here and in the full `{:catch}` section, a space then a newline in
  the other variants.
- `unformatted_ours_space_body.svelte` — the catch shorthand with a space-only body
  (`{#await fn(…) catch} {/await}`), which renders nothing and trims away.
- `unformatted_ours_catch_section.svelte` — the section written in full with a space-only body
  (`{#await fn(…)}{:catch} {/await}`); tsv folds it into the head shorthand.
- `unformatted_ours_space_pending.svelte` — a space-only pending before the space-only catch
  (`{#await fn(…)} {:catch} {/await}`), which folds away too.
- `unformatted_ours_dropped_then.svelte` — a space-only `{:then}` before the space-only catch
  (`{#await fn(…) then value} {:catch} {/await}`), which renders nothing and drops, marker and
  binding, so the catch folds into the head.
- `variant_hugged_parents.svelte` — prettier's output from every `unformatted_ours_*`: the empty
  `{:catch}` deleted and the `{#if}` and `<span>` kept hugged as authored, a form both formatters
  hold (with no catch there is no forced break for tsv to propagate).
- `output_prettier.svelte` — prettier deletes the empty `{:catch}` from every authoring, leaving
  the section-less `{#await fn(…)}{/await}` on one line (it never wraps a block head either).

A render-free body decides nothing about the layout: a space-only catch body prints the same
empty section as a glued one, so both settle on the hugged form. A newline-authored body keeps the
block form, the empty body's blank line between `catch}` and `{/await}`, as for any empty kept
section ([empty_catch_multiline](../empty_catch_multiline_prettier_divergence/)).

## Reason

Content preservation (prettier deletes an empty `{:catch}`, which changes what a rejected promise
does) and print width (tsv wraps a block head past 100 columns where prettier never does — see
[conformance_prettier.md §Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy)).
The entry is
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).
