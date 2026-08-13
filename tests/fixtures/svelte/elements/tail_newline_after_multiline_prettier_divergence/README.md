# tail_newline_after_multiline

An authored newline between an inline element's/component's closing tag and following
sibling text is **preserved when the element renders multiline**, and **reflowed when it
renders inline** — the boundary's layout follows the unit's rendered layout, not the
spelling alone:

- `</a>⏎text2` after a multiline-rendering `<a>` keeps the text on its own line (the
  author separated the prose from the tag pile; both forms are clean, so the authored
  one is kept — Tier-2). The **space** spelling stays hugged (`</a> text3.`), so the
  boundary is dual-stable there: `variant_welded` pins the hugged form at the terminal
  boundaries, where prettier holds it too.
- After an element that renders **inline**, the newline is a spelling difference only
  and reflows with the fill (`text4 <a href="/z">{expr3}</a> text5` — the fitting
  control): `prettier_variant_fitting_newline` pins the newline authoring, which
  prettier keeps stable and tsv packs.

The mechanism is a render-time probe (`flow_break_probe` on the element,
`hold_line_after_broken_flow` on the tail's fill): the fill's leading line renders as a
forced break exactly when the element's subtree actually emitted one — layout-keyed at
render, with no measurement change anywhere. Prettier
preserves the authored spelling at every one of these boundaries regardless of the
element's layout (with one exception it normalizes itself: a **non-terminal**
space-hugged tail after a multiline element, `</a> text2⏎<a…`, which prettier moves to
its own line — tsv keeps that space hugged, dual-stable. Prettier's own-line target is
the newline spelling the rule above preserves, so tsv holds *both* forms and only the
hug side is one-sided; not pinned here — the re-break is
[inline_wide_content_text_sibling_long](../inline_wide_content_text_sibling_long_prettier_divergence/)'s
`output_prettier` claim).

The divergences: tsv packs the fitting case's newline authoring where prettier
preserves it, and tsv's welded terminal tails (`</a> text3.`, `</Comp> text8.`) are its
own terminal-hug stance. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
