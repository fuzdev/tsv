# nested_script_style_glued_before_tag_follower_prettier_divergence

A frozen nested `<script>` / `<style>` whose directive is glued to the text before it, with
whitespace between the element and the tag that follows it — an inline element, a component or an
expression tag. The freeze pins the element's bytes; the boundary after it is the printer's, and
tsv gives it the form the same element gets unfrozen — a line break. The directive stays glued in
front, so the element keeps the line it shares with the text before it.

Prettier breaks the frozen body open (`<script>⏎let  b  =  2;⏎</script>`) and converges.
Before a component or an expression tag it breaks after the element too; before an inline element
it keeps each spelling of that whitespace as its own stable form — the line break, and the space
of the variant below. tsv keeps the body bytes it is given, so it keeps prettier's output as
written and converges prettier's space on the break.

- `output_prettier.svelte` — prettier's output: the frozen bodies broken open, the break after
  each element kept. tsv keeps it as written — the bytes prettier moved into the body are frozen
  bytes to tsv, and the break is the one it gives that boundary.
- `unformatted_ours_compact.svelte` — each cell on one line, a space after the element; tsv
  normalizes it to `input.svelte`, prettier to `divergent_variant_space_after.svelte`.
- `unformatted_ours_spaces.svelte` — extra spaces around the element's follower and at the
  parents' content boundaries; tsv normalizes it to `input.svelte`, prettier to
  `divergent_variant_space_after.svelte`.
- `divergent_variant_space_after.svelte` — prettier's stable form of those authorings: the
  bodies broken open, and a space kept after each element an inline element follows. tsv
  rewrites it to `output_prettier.svelte`, not to `input.svelte` — it keeps the body bytes prettier
  broke open, and gives the boundary after the element its break.

All of them render identically.

## Reason

Content preservation and convergence, render-free under Svelte 5. A nested `<script>` /
`<style>` renders no box, so the content on its two sides meets across the whitespace after it:
`text5<!-- prettier-ignore --><script>…</script> <b>inline1</b>` renders `text5 inline1`, and a
line break there renders the same one space. That is what licenses the element's own line
unfrozen, and the break after it is the one a frozen element owes too.

See
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy)
and
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

## Related

- [nested_script_style_glued_before](../nested_script_style_glued_before_prettier_divergence/) —
  the same boundary before text, where prettier never converges
- [declaration_glued_before](../declaration_glued_before_prettier_divergence/) — the same
  boundary after a frozen declaration tag
