# pre_await_whitespace_section_prettier_divergence

An `{#await}` inside `<pre>` with a section whose content is whitespace only — a pending branch or
a `{:then}` body of spaces, a tab, a newline. Outside a whitespace-preserving element the compiler
trims such a section to nothing, so it renders exactly like an absent one: a pending branch may
fold into the `then` / `catch` head shorthand, and an empty `{:then}` is dropped. Inside `<pre>`
Svelte's compiler carries its whitespace-preserving state into every descendant fragment — an
inline element's, a block's, a component's and a special element's children included (the last
four cases) — so the section is rendered text: the pending branch is what the page shows while the
promise is pending, the `{:then}` body what it shows once it resolves. tsv keeps every section as
written.

- `input.svelte` — tsv's form, every section as authored. The newline-authored pending is the
  control: it never folds, outside `<pre>` either.
- `unformatted_ours_catch_first.svelte` — the two catch-first rows written with the `{:catch}`
  section first; tsv prints the then section first, as everywhere, and keeps the whitespace-only
  section in place.
- `output_prettier.svelte` — prettier folds every whitespace-only pending into the shorthand and
  drops every whitespace-only `{:then}`, deleting the whitespace both times; for the empty
  `{:catch}` case it deletes the catch section too
  ([empty_catch](../../blocks/await/empty_catch_prettier_divergence/)).

## Reason

Prettier bug and content preservation: deleting rendered text changes the page. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
