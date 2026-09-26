# pre_raw_text_close_containers_prettier_divergence

A nested `<script>` / `<style>` inside `<pre>` keeps its closing tag whole at every depth: under
an inline element, in a block body, in a component, in a `<pre>` nested inside other elements,
glued to a second element of its own kind, and with a comment glued after it. The glued pair is
where a split is worst: Svelte ends a nested raw-text body at the first literal `</script>` /
`</style>`, so splitting the **second** closing tag leaves a file that does not parse, and
splitting the **first** one parses as a single element whose body runs on to the second closing
tag — the two elements silently become one (pinned for a pair of scripts and a pair of styles).
The break goes before an opening tag's `>` instead.

- `input.svelte` — tsv's form.
- `unformatted_ours_compact.svelte` — every case on one line; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier formats each raw-text body onto its own indented lines.

## Reason

Design choice: inside `<pre>` a nested raw-text body is the author's text and tsv keeps it
byte-for-byte; the closing tag stays whole because Svelte's parser requires it. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
