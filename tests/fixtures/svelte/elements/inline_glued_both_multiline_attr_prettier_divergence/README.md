# inline_glued_both_multiline_attr_prettier_divergence

An inline element glued to text on **both** sides whose **own** attribute forces its opening tag to
break — a block-bodied handler, a quoted value or template literal spanning lines, a block comment
spanning lines — lays out its content **block-style**: the `>` on its own line, the content on its
own indented line, the closing tag on the next. That is the form its spaced twin
([multiline_value_inline](../../attributes/multiline_value_inline_prettier_divergence/)) and a
`<svelte:element>` in the same position already take. The closing-`>` dangle a glued-both element
otherwise takes keeps its content on the tag's line, so it has no form for a tag that has already
broken. The cases cover an element, a component and a `<svelte:element>`; text, element and
expression content, and content long enough to wrap; inside a block element, inside an inline
`<span>` and at the root. Only a **forced** break counts: an attribute authored across lines that
prints flat forces nothing, so its element stays on one line (the last two cases, where prettier
agrees), and an attribute list that merely wraps at print width is not this rule.

Prettier hugs the content to the last attribute and dangles the closing `>` onto the following
text's line (`}}>inline</button⏎\t>text2`), and keeps the block-style form stable too, so the
divergence is one of normalization:

- `unformatted_ours_compact.svelte` — each case authored on one line (bar the multi-line
  attribute), and the flat cases' attributes authored across lines; tsv normalizes it to
  `input.svelte`, prettier to its dangled form.
- `unformatted_ours_spaces.svelte` — the same with padded tags and content boundaries, which the
  compiler trims.
- `unformatted_ours_hug.svelte` — the content hugged onto the attributes' `>` line
  (`}}⏎\t>inline</button>text2`) in every forced case, and the flat cases' attributes authored
  across lines in the same hugged shape; tsv normalizes it to `input.svelte`.
- `prettier_variant_dangle.svelte`, `prettier_variant_spaces.svelte`,
  `prettier_variant_hug.svelte` — prettier's stable forms of the compact, spaced and hugged
  authorings. The spaced one is block-style and keeps the padded content boundaries of the
  element child and the flat cases; the hugged one is the dangled form except inside the inline
  `<span>`, whose own tags it keeps block-style where the compact authoring dangles them too.
  tsv normalizes each to `input.svelte`.

Every form renders identically: the content boundaries are trimmed at compile, and every glued
boundary stays glued.

## Reason

Design choice: content that goes multiline lays out block-style whatever made it multiline, and an
element whose opening tag has already broken has no inline form for its content. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
(§Moving a run: break-before, travel, and welded units — the glued-both bounds) and the shared
frame's
[§Authoring Convergence Philosophy](../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).

## Related

- [inline_glued_both_breaking_content_prettier_divergence](../inline_glued_both_breaking_content_prettier_divergence/)
  — the same bound reached by content that breaks, including a CHILD's multi-line attribute
- [inline_glued_both_dangle_long](../inline_glued_both_dangle_long/) — the dangle this bounds, on
  an opening tag that stays on one line
- [multiline_value_inline_prettier_divergence](../../attributes/multiline_value_inline_prettier_divergence/)
  — the spaced twin
- [pre_glued_inline_multiline_attr](../pre_glued_inline_multiline_attr/) — the same authoring
  inside `<pre>`, where the boundaries render and the dangle stays
