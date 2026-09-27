# nested_script_style_glued_prettier_divergence

A frozen nested `<script>` / `<style>` glued to content on both sides. The directive stays glued
to the text in front of it, and the boundary after the frozen element is the one the element gets
unfrozen: a nested `<script>` / `<style>` renders no box, so glued to content on both sides it
keeps both glued boundaries
([nested_script_style_glued](../../../elements/nested_script_style_glued/)), and frozen it does the
same. A break after the element would render a space between `text1` and `text2`.

Prettier breaks the frozen body open and then adds a blank line before the parent's closing tag
on every pass, so it **never converges** here (`prettier_nonconvergent.txt`) and there is no oracle to
compare against; tsv's claim is the one every fixture makes — `input.svelte` formats to itself.

## Reason

Content preservation and F1: the frozen bytes are the author's, the glued boundaries are a render
fact, and the boundary after the frozen node takes the form the node takes there unfrozen.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

## Files

- `unformatted_ours_spaces.svelte` — extra spaces at the parents' content boundaries; tsv
  normalizes it to `input.svelte`.
- `prettier_nonconvergent.txt` — prettier adds a blank line before the parent's closing tag on
  every pass, so no `output_prettier.svelte` exists.

## Related

- [block_glued_before](../block_glued_before_prettier_divergence/) — the boundary after a frozen
  block element, which owns its line
- [directive_gap_glued](../directive_gap_glued/) — the glued gap in front of inline kinds
