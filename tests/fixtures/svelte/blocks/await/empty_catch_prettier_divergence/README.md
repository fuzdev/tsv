# empty_catch_prettier_divergence

An `{#await}` with an **empty-body `:catch`** — `{#await promise catch error}{/await}`,
`{#await promise}x{:catch error}{/await}`. Prettier **deletes** the empty catch section
(down to `{#await promise}{/await}` / `{#await promise}x{/await}`); **tsv keeps it**.

This is a deliberate **correctness** divergence. An empty `{:catch}` still *handles* a
rejected promise (renders nothing); deleting it lets the rejection **propagate** instead —
a behavior change, not a formatting one. So tsv preserves the section even though its body
is empty. (An empty `:then` carries no such meaning — its `value` binding is unused when
nothing renders — so tsv **drops** an empty `:then`, matching prettier. Only `:catch` diverges.)

- **catch-shorthand** — `{#await promise catch error}{/await}` (kept)
- **empty `:catch` after content** — `{#await promise}x{:catch error}{/await}` (kept)
- **inline sibling** — an `<a>` directly before an empty-catch await keeps its closing `>`
  hugged (`</a>{#await …}`); the `>` is placed, never dropped (a dropped `>` is unreparseable)

`unformatted_ours_spaces.svelte` authors the sections with spaces
(`{#await promise} {:catch error}{/await}`); tsv folds + normalizes it to the `input.svelte`
shorthand (this is the authoring that exercises the space-only layout path). Prettier would
drop the catch instead, so it does not normalize to `input.svelte` — hence `unformatted_ours_*`.

## Reason

Correctness over conformance: an empty `:catch` is a rejection handler, so removing it changes
runtime behavior. tsv keeps it where prettier drops it. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [await/empty_sibling_gt](../empty_sibling_gt/) — a bare empty `{#await promise}{/await}` after a sibling (`>` hugged, in parity with prettier — not a divergence)
- [await/whitespace_internal](../whitespace_internal/) — sections with real content + surrounding whitespace (not a divergence)
