# nested_script_style_glued_block_style_prettier_divergence

A nested `<script>` / `<style>` is an ordinary element, not a hoisted section, and it renders no
box — so the content on either side of it meets directly. Glued to content on **both** sides
(`text1<script>…</script>text2` renders `text1text2`), neither glued boundary may take a break:
the element lays out the way a glued inline element with a breaking body does — both tags intact
and glued to their neighbours, the body on its own indented lines. A comment or a declaration tag inside the
run renders nothing either, so the run stays glued through it.

- **Inline element, component and block-body parents → block-style.** The fragment around the
  glued run goes block-style, as it does around any inline element that renders multiline.
  Prettier keeps that form when it is authored (`output_prettier.svelte` differs only in the last
  case), but from the one-line authoring (`unformatted_ours_compact.svelte`) it dangles the
  parent's delimiters (`<span⏎\t>text1<script>…</script>text2</span⏎>`) or hugs a block body
  (`{#if cond}text1<script>…</script>text2{/if}`) — `prettier_variant_dangle.svelte`, stable as
  such. tsv normalizes both to `input.svelte`.
- **A word in front of the glued run → the run starts its own line.** The space before
  `text2<script>` is inter-node whitespace, and the unit after it renders multiline, so tsv spends
  the space on a line break (`text1⏎text2<script>`). Prettier keeps `text1 text2<script>`
  (`output_prettier.svelte`).

All of these render identically: the content boundaries of the parents are trimmed at compile, the
space before the run collapses to one rendered space whichever way it is spelled, and the glued
boundaries are never split.

## Reason

Design choice, render-free under Svelte 5 — the layout an inline element glued on both sides
already takes, as for a global `svelte:*` element in
[global_glued_both_sides_wide_long](../../special_elements/global_glued_both_sides_wide_long_prettier_divergence/),
and the multiline unit's drop to a fresh line of
[inline_sibling_space_before_bounding](../inline_sibling_space_before_bounding_prettier_divergence/).
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [nested_script_style_glued](../nested_script_style_glued/) — the block-parent cells, where both
  formatters agree
- [nested_script_style_glued_long](../nested_script_style_glued_long/) — text after the closing
  tag wraps at print width
