# pre_raw_text_close_long_prettier_divergence

A nested `<script>` / `<style>` inside `<pre>` whose line is too wide. Svelte reads a nested
raw-text element's body up to the first literal `</script>` / `</style>` — no whitespace before
the `>`, unlike a top-level one or a `<textarea>` — so its closing tag cannot split: `</script⏎>`
does not parse. tsv keeps the closing tag whole and spends the break before the opening tag's `>`
instead, the shape a whitespace-sensitive element already takes when its content ends on
whitespace ([ws_sensitive_head_content_edges](../ws_sensitive_head_content_edges/)). At exactly
100 columns nothing moves; content that opens on whitespace offers neither delimiter a break, so
that line runs over, and so does an empty body, which prints like any empty element
(`<strong></strong>`).

- `input.svelte` — tsv's form; the break lands inside the opening tag, so the text inside `<pre>`
  is untouched.
- `unformatted_ours_compact.svelte` — every case on one line; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier formats each raw-text body onto its own indented lines.

## Reason

Design choice: inside `<pre>` every byte outside template syntax is the author's own, and a
nested raw-text body is text, so tsv keeps it byte-for-byte where prettier reformats it; the
closing tag stays whole because Svelte's parser requires it. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
