# empty_catch_multiline_prettier_divergence

The **block (multiline)** counterpart to [await/empty_catch](../empty_catch_prettier_divergence/):
an `{#await}` with an **empty-body `:catch`** whose sibling sections render block-style. Prettier
**deletes** the empty catch; **tsv keeps it**.

Keeping it does **not** mean giving it a line: the empty section still **collapses**, so its marker
glues to the close — `{:catch error}{/await}` — exactly as prettier collapses an empty `{:else}` to
`{:else}{/if}` and as every other empty construct behaves. A section that carries content keeps its
own indented line and `{/await}` drops to its own line as usual.

This is the same deliberate **correctness** divergence as the inline case: an empty `{:catch}` still
*handles* a rejected promise (renders nothing), so deleting it lets the rejection **propagate** — a
behavior change, not a formatting one. (An empty `:then` carries no such meaning and is dropped,
matching prettier; only `:catch` diverges.)

- **empty `:catch` after a multi-node pending body**
- **full form (pending + then bodies), empty `:catch`**

**One fixed point, however the block was authored.** All three authorings converge on `input.svelte` —
the block layout is reached either by authoring it across lines or by *breaking*, and the empty catch
collapses the same way in both:

- `unformatted_ours_blank.svelte` — a blank line authored inside the empty catch body
  (`{:catch error}⏎⏎{/await}`); the section is whitespace-only and render-free, so tsv drops it and
  the marker glues.
- `unformatted_ours_space_only.svelte` — the same document authored **inline** (bodies glued, a space
  in the catch), which reaches the block layout by breaking.

Prettier deletes the catch in every case, so neither normalizes to `input.svelte` — hence
`unformatted_ours_*`.

`divergent_variant_body_hug.svelte` is prettier's own stable output for the inline authoring: it
deletes the catch **and** hugs the body to the opening tag (`{#await promise}<p>a</p>⏎\t<p>b</p>{/await}`),
prettier's boundary-whitespace-driven block-body layout. tsv rewrites that form to its uniform
body-drop (a distinct third stable form) — see
[§Svelte: Blocks — uniform body drop](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Reason

Correctness over conformance: an empty `:catch` is a rejection handler, so removing it changes runtime
behavior. tsv keeps it — inline or block-style — while still collapsing it like any empty section. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [await/empty_catch](../empty_catch_prettier_divergence/) — the inline-form empty-catch divergence
- [await/whitespace_mixed](../whitespace_mixed/) — per-body boundary-whitespace preservation in the
  space-only layout (not a divergence)
