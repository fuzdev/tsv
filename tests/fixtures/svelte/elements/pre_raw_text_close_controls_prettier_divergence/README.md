# pre_raw_text_close_controls_prettier_divergence

What the whole-closing-tag rule for a nested `<script>` / `<style>` inside `<pre>` does **not**
reach, each at 101 columns: the parent `<b>` of a script still dangles its own closing `>`; a
component named `Script` and an upper-case `<SCRIPT>` (whose body Svelte parses as markup) are no
raw-text elements and dangle theirs; and a script whose body already ends on whitespace keeps the
break before its opening `>` that the family gives such content
([ws_sensitive_head_content_edges](../ws_sensitive_head_content_edges/)). Svelte reads each of
these end tags as an ordinary one, so the split parses.

- `input.svelte` — tsv's form.
- `unformatted_ours_compact.svelte` — every case on one line; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier formats the two lower-case scripts' bodies onto their own
  indented lines; the other two cases it prints as tsv does.

## Reason

Design choice: inside `<pre>` a nested raw-text body is the author's text and tsv keeps it
byte-for-byte. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
