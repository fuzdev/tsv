# nested_script_style_head_own_line_prettier_divergence

A nested `<script>` inside `<svelte:head>`, between a `<title>` and a `<meta>`. The compiler hoists
the `<title>` out of the fragment, so the script has only the head's content edge on that side,
and a global element's content joins no line of the page: a break there is not rendered, and the
script takes its own line, as a nested `<script>` / `<style>` does wherever the line is
render-free
([nested_script_style_own_line](../nested_script_style_own_line_prettier_divergence/)).

- `unformatted_ours_glued.svelte` — every head boundary glued, the body lines kept: tsv normalizes
  it to `input.svelte`.
- `prettier_variant_glued.svelte` — prettier's stable form of that authoring: the head's delimiters
  dangle around the glued children. tsv normalizes it to `input.svelte` too. Prettier also keeps
  `input.svelte` as written, so there is no `output_prettier.svelte`.

All of them render identically.

## Reason

Design choice: the own line is render-free at the head's content edge, and a hoisted `<title>` is
not a neighbour. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
