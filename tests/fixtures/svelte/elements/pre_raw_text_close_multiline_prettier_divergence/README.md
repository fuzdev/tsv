# pre_raw_text_close_multiline_prettier_divergence

A nested `<script>` / `<style>` inside `<pre>` whose body holds a line break and meets both of its
tags on visible bytes. The line break forces the element open at any width, and the break it needs
lands before the opening tag's `>`; the closing tag stays whole, because Svelte ends a nested
raw-text body at the first literal `</script>` / `</style>` and `</script⏎>` does not parse. The
cases: the only child, one with an attribute between text, and a `<style>`.

- `input.svelte` — tsv's form; the body's bytes are untouched.
- `unformatted_ours_compact.svelte` — every opening tag written whole; tsv normalizes it to
  `input.svelte`.
- `output_prettier.svelte` — prettier formats each raw-text body onto its own indented lines.

## Reason

Design choice: inside `<pre>` a nested raw-text body is the author's text and tsv keeps it
byte-for-byte; the closing tag stays whole because Svelte's parser requires it. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
