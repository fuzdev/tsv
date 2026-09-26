# pre_raw_text_close_attrs_prettier_divergence

The opening tag of a nested `<script>` / `<style>` inside `<pre>`, with the closing tag held
whole. The attribute list wraps as it does on any whitespace-sensitive element, and the `>` takes
its own line one level in rather than the closing tag splitting (`</script⏎>` does not parse:
Svelte ends a nested raw-text body at the first literal `</script>` / `</style>`). The cases: a
wrapping list with and without text after it, one attribute that fits (only the `>` moves), a body
tsv never formats (`type="application/ld+json"`), a list ending on a `//` (the `>` cannot share
its line), and content opening on whitespace, where the `>` hugs the last attribute exactly as it
does in [ws_sensitive_head_content_edges](../ws_sensitive_head_content_edges/).

- `input.svelte` — tsv's form.
- `unformatted_ours_compact.svelte` — every tag on one line; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier formats each raw-text body onto its own indented lines and
  gives the `>` its own line at the tag's indent; behind the `//` it deletes the body outright.

## Reason

Design choice, and content preservation in the `//` case, where prettier drops the body: inside `<pre>` a nested raw-text body is the author's text and tsv keeps it
byte-for-byte; the closing tag stays whole because Svelte's parser requires it. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
