# nested_script_style_own_line_prettier_divergence

A nested `<script>` / `<style>` with a body takes its **own line** wherever that is render-free:
the boundary on either side already holds whitespace (here a space on both sides), so a line break
there collapses to the same single rendered space. It is the declaration tag's and the global
`svelte:*` element's rule — own line unless glued to content on both sides, where the glue is kept
([nested_script_style_glued](../nested_script_style_glued/)).

- `input.svelte` — tsv's form, one the author can also write: prettier keeps it, so there is no
  `output_prettier.svelte`.
- `unformatted_ours_compact.svelte` — each case authored on one line with spaced boundaries; tsv
  normalizes it to `input.svelte`.
- `prettier_variant_inline.svelte` — prettier's stable form of that authoring: the element stays
  inline beside its spaced neighbours (the inline parents dangle their delimiters). tsv
  normalizes it to `input.svelte` too.

All of them render identically.

## Reason

Design choice, render-free under Svelte 5: a nested `<script>` / `<style>` renders no box, and a
whitespace boundary beside it collapses to one rendered space however it is spelled. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
