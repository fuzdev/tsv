# nested_script_style_glued_before_prettier_divergence

A frozen nested `<script>` / `<style>` whose directive is glued to the text before it, with
whitespace between the element and the text that follows it, in an element and in a block body.
The freeze pins the element's bytes; the boundary after it is the printer's, and tsv gives it the
form the same element gets unfrozen — a line break. The directive stays glued in front, so the
element keeps the line it shares with the text before it.

Prettier breaks the frozen body open, and with text following the element it then adds a blank
line before the parent's closing tag on every pass, so it **never converges** on this input
(`prettier_nonconvergent.txt`) and there is no oracle to compare against; tsv's claim is the one
every fixture makes — `input.svelte` formats to itself.

## Reason

Content preservation and F1, render-free under Svelte 5. A nested `<script>` / `<style>` renders
no box, so the content on its two sides meets across the whitespace after it:
`text1<!-- prettier-ignore --><script>…</script> text2` renders `text1 text2`, and a line break
there renders the same one space. That is what licenses the element's own line unfrozen, and the
break after it is the one a frozen element owes too. The whitespace is never deleted:
`…</script>text2` renders `text1text2`.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

## Files

- `unformatted_ours_compact.svelte` — each cell on one line, a space after the element; tsv
  normalizes it to `input.svelte`.
- `unformatted_ours_spaces.svelte` — extra spaces around the text after the element and at the
  parents' content boundaries; tsv normalizes it to `input.svelte`.
- `prettier_nonconvergent.txt` — prettier adds a blank line before the parent's closing tag on
  every pass, so no `output_prettier.svelte` exists.

## Related

- [nested_script_style_glued_before_tag_follower](../nested_script_style_glued_before_tag_follower_prettier_divergence/)
  — the same boundary before an inline element, a component or an expression tag, where prettier
  converges
- [nested_script_style_glued](../nested_script_style_glued_prettier_divergence/) — the same
  element glued on both sides, which owes no break
- [declaration_glued_before](../declaration_glued_before_prettier_divergence/) — the same
  boundary after a frozen declaration tag
