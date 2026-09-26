# pre_special_element_whitespace_only_prettier_divergence

A special element inside `<pre>` whose content is whitespace only — spaces, a lone newline, a
tab. Inside `<pre>` that whitespace is rendered text: Svelte's compiler carries its
whitespace-preserving state into every child fragment, a special element's included, so the
served page holds the run as written. tsv keeps it, as it already does for an inline element and
a component in the same position (the last two cases).

- `input.svelte` — tsv's form, the content as authored.
- `unformatted_ours_tag_split.svelte` — every tag broken before its `>`; tsv normalizes it to
  `input.svelte`.
- `unformatted_ours_spaces.svelte` — extra spaces inside every tag; tsv normalizes it to
  `input.svelte`.
- `output_prettier.svelte` — prettier deletes the whitespace in every case, so `a   c` renders as
  `ac`.

## Reason

Prettier bug and content preservation: deleting rendered text changes the page. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
