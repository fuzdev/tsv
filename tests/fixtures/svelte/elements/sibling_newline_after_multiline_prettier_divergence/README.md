# sibling_newline_after_multiline

An authored newline between a unit that **renders multiline** and the inline sibling after
it — an element, a component, a tag, or a void — is **preserved**, exactly as it is before
sibling text ([tail_newline_after_multiline](../tail_newline_after_multiline_prettier_divergence/)):
the boundary's layout follows the unit's rendered layout, not the spelling alone.

- `</span>⏎<b>inline2</b>`, `</Comp1>⏎<Comp2 />`, `</a>⏎{expr2}`, `</code>⏎<input />` after a
  multiline-rendering unit keep the sibling on its own line (the author separated it from the
  tag pile; both forms are clean, so the authored one is kept — Tier-2). The **space** spelling
  stays the per-width hug, so the boundary is dual-stable there: `variant_space` pins the hugged
  form at the element and void tails, where prettier holds it too (a hugged component or tag is
  prettier's `isInlineElement` split, pinned by
  [inline_adjacent_component_after_multiline](../inline_adjacent_component_after_multiline_prettier_divergence/)
  and not repeated here).
- After a unit that renders **inline**, the same newline is a spelling difference only and
  reflows with the fill — the fitting control, and the held sibling's own text tail
  (`<b>inline2</b> text2`): `prettier_variant_fitting_newline` pins those newline authorings,
  which prettier keeps stable and tsv packs. After a **breaking tag** the sibling hugs the
  tag's closing brace (`)} <b>inline6</b>`): the tag's break lands inside its own expression,
  so the tag-pile reading does not arise, and prettier's own-line authoring of it is in the
  same variant.
- `unformatted_glued_head` glues each `<p>`'s opening tag to its text (`<p>text1`),
  reaching the same boundary without content-boundary air; both formatters normalize it to
  the input.

The mechanism is the render-time probe the text-tail rule uses (`flow_break_probe` on the
unit, the hold on the sibling's leading line): the boundary renders as a forced break
exactly when the unit's subtree actually emitted one — layout-keyed at render, with no
measurement change anywhere. Prettier preserves the authored spelling at every one of these
boundaries regardless of the unit's layout, so the multiline cases are agreement.

The divergences: tsv packs the fitting cases' newline authorings where prettier preserves
them, and hugs the sibling after a breaking tag. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
