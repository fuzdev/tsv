# sibling_newline_after_multiline_long

The width razor for the layout-keyed sibling boundary
([sibling_newline_after_multiline](../sibling_newline_after_multiline_prettier_divergence/)):
the same authored newline before an inline element, one character apart.

- **100** — the component's inline line is exactly print width, so it renders inline; the
  authored newline after it is a spelling the fill reflows, and the `<b>` sits on its own
  line by **width** alone.
- **101** — one character wider, the component renders multiline (block-style), and the
  authored newline is **preserved**: the `<b>` keeps its own line where a **space** would
  hug (`variant_welded`, dual-stable — prettier holds the hugged form too).

The decision is render-keyed (`flow_break_probe` on the component, the hold on the
sibling's leading line): the unit's rendered layout, not its authored spelling, selects
whether the boundary's newline is intent — which is what makes the two cases differ by
nothing but one padding character. Prettier preserves the authored newline in every case
here, so both `input` forms are agreement; the divergence family is cataloged with the
parent fixture.

See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
