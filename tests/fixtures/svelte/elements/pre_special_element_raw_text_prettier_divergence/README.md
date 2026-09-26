# pre_special_element_raw_text_prettier_divergence

A nested `<script>` / `<style>` among a special element's content inside `<pre>`. The special
element's content is `<pre>` content, so the raw-text body is kept byte-for-byte and its closing
tag stays whole, the rule a nested raw-text element already follows directly under `<pre>`. When
the line is too wide the break goes before the special element's `>` and its closing `>` dangles.

- `input.svelte` — tsv's form.
- `unformatted_ours_compact.svelte` — every case on one line; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier formats each raw-text body onto its own indented lines.

## Reason

Design choice: inside `<pre>` a nested raw-text body is the author's text. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
