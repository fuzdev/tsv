# empty_catch_no_binding_prettier_divergence

An `{#await}` with an **empty `{:catch}` that binds nothing** — the bare-section twin of
[await/empty_catch](../empty_catch_prettier_divergence/), whose sections all bind an `error`.
Prettier **deletes** the section; **tsv keeps it**.

The section carries no binding and no content, but its presence is behavior: without a catch
section Svelte's client runtime rethrows the rejection, while an empty one handles it and renders
nothing. The compiled component passes a catch callback in the first case and none in the second,
so deleting the section changes what a rejected promise does.

- **catch-shorthand, empty body** — `{#await promise catch}{/await}`
- **after a pending body** — `{#await promise}text{:catch}{/await}`
- **after a then-shorthand body** — `{#await promise then value}{value}{:catch}{/await}`
- **block form** — the kept section keeps its own line, `{:catch}` and `{/await}` each on a line
  with the empty body's blank line between, exactly as for `{:catch error}`
  ([await/empty_catch_multiline](../empty_catch_multiline_prettier_divergence/))
- **after a newline-authored empty pending branch** — the block form, the pending branch and the
  catch each keeping an empty line
- **inside `<pre>`** — the same section in the whitespace-sensitive layout, and a space-only body,
  which there is rendered text and stays

`unformatted_ours_space_body.svelte` gives every empty catch outside `<pre>` a space-only body,
which is render-free and trims away, so the section must survive as an empty one rather than drop on
the next pass. `unformatted_ours_glued.svelte` glues both block-form catches to the close
(`{:catch}{/await}`, the authoring with no body node at all), the second behind a single-newline
pending. `unformatted_ours_catch_first.svelte` writes the then-shorthand case catch-first
(`{#await promise catch}{:then value}{value}{/await}`), which tsv folds into the `then` shorthand
with the catch following the body. Prettier deletes the catch from all three.

- `output_prettier.svelte` — prettier deletes every empty `{:catch}`.

## Reason

Prettier bug and content preservation: deleting the section changes runtime behavior. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).
