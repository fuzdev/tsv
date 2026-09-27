# nested_script_style_glued_inline_edge_prettier_divergence

A nested `<script>` / `<style>` glued to content on one side and to the content edge of an
**inline** parent on the other. The element renders no box, and it is not hoisted, so the
compiler's edge trim stops at it: whitespace between the content and the element stays, and at an
inline parent's edge the line goes on past the parent — `text1<span>text2<script>…</script></span>text3`
renders `text1text2text3`, and a break before the element would render a space after `text2`. So
the element keeps its glue, laid out as a glued inline element whose body breaks. The same holds
through a comment (it renders nothing), between two such elements, in a component or a block body
inside a line, and when the only whitespace on the far side reaches the parent's edge, where the
compiler trims it. Its body breaks, so an inline parent glued to text on both sides lays out
block-style around it, as it does around any content that cannot stay inline — around a lone
element between the parent's two edges, and around a block holding a pair of them, with each
`<style>` body one level past its own tag.

- `unformatted_ours_one_line.svelte` — each case on one line, the trimmed-whitespace case with its
  authored space before `</span>`; tsv normalizes it to `input.svelte`. The block's pair is written
  `</script> <style>`, spaced: glued, it takes its own lines inside the block, which renders nothing
  but reads as a render change to the validator's render key (whitespace never collapses across a
  nested element there), so `tests/svelte_nested_script_style_own_line.rs` pins a glued pair in a
  block parent.
- `prettier_variant_dangle.svelte` — prettier's stable form of that authoring: it keeps the glue
  too, and dangles the inline parents' delimiters; tsv normalizes it to `input.svelte`. Prettier
  keeps `input.svelte` as written.
- `unformatted_ours_hug.svelte` — the inline parents hugged around the breaking body, their
  content at the container's indent (`text1<span><script>⏎…⏎</script>text2</span>text3`); tsv
  normalizes it to `input.svelte`.
- `prettier_variant_hug.svelte` — prettier's stable form of that authoring, which indents the
  content and dangles each delimiter the hug had glued; tsv normalizes it to `input.svelte`.

All of them render identically.

## Reason

Design choice for the layout — the one a glued inline element whose body breaks takes in these
positions — and a render fact for the glue. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [inline_glued_both_breaking_content](../inline_glued_both_breaking_content_prettier_divergence/)
  — an inline parent glued to text on both sides around any content that breaks
- [nested_script_style_glued_block_style](../nested_script_style_glued_block_style_prettier_divergence/)
  — glued to content on both sides
- [nested_script_style_own_line](../nested_script_style_own_line_prettier_divergence/) — where the
  element's own line is render-free
