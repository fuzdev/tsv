# inline_glued_both_breaking_content_prettier_divergence

An inline element glued to text on **both** sides whose content breaks however it is laid out —
a child element holding a control-flow block, an `{#await}` branch holding block elements, a
snippet whose body breaks, a child whose attribute value spans lines, a multi-line comment — lays
out **block-style**:
both tags intact, the content on its own indented lines. That is the form its spaced twin, and an
inline element holding the block directly, already take. The closing-`>` dangle a glued-both
element otherwise takes keeps its content inline, so it has no form for content that cannot be.

Around a breaking block, attribute or snippet prettier dangles every delimiter instead
(`<span⏎\t>text2 <strong⏎\t\t>{#if cond}…{/if}</strong⏎\t></span⏎>`); around a multi-line
comment it keeps the hug as written (`text1<span>text2 <!-- a⏎b --> text3</span>text4`). It keeps
the block-style form stable as well, so the divergence is one of normalization:

- `unformatted_ours_compact.svelte` — each case authored on one line; tsv normalizes it to
  `input.svelte`, prettier to its dangled form.
- `unformatted_ours_spaces.svelte` — the same with padded content boundaries, which the compiler
  trims.
- `unformatted_ours_hug.svelte` — the tags hugged around the break, the content at the
  container's indent (`text1<span>text2⏎<strong>…</strong></span>text4`); tsv normalizes it to
  `input.svelte`.
- `prettier_variant_dangle.svelte`, `prettier_variant_spaces.svelte`,
  `prettier_variant_hug.svelte` — prettier's stable forms of the compact, spaced and hugged
  authorings (the last keeps the hugged content's lines inside its dangled delimiters). The
  compact and spaced forms keep all three multi-line comment cases hugged; the hugged form keeps
  only the two where the comment is a direct child, and dangles the one inside `<em>`. tsv
  normalizes each to `input.svelte`.

Every form renders identically: the content boundaries are trimmed at compile, and every glued
boundary stays glued.

## Reason

Design choice: content that goes multiline lays out block-style whatever made it multiline. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
(§Moving a run: break-before, travel, and welded units — the glued-both bounds).

## Related

- [inline_glued_both_dangle_long](../inline_glued_both_dangle_long/) — the dangle this bounds, on
  content that stays inline
- [nested_script_style_glued_inline_edge_prettier_divergence](../nested_script_style_glued_inline_edge_prettier_divergence/)
  — the same bound reached by a nested `<script>` / `<style>` whose body breaks
