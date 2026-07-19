# inline_boundary_whitespace_prettier_divergence

An inline element's **content-boundary whitespace** is render-free under Svelte 5 — the
compiler trims every fragment edge at compile (`clean_nodes`), so `<span> text </span>`
renders identically to `<span>text</span>`. tsv therefore trims it even when the content
fits inline: `<span> text </span>` → `<span>text</span>`, uniformly with block elements
and components (which already trim). A boundary run is trimmed whole (spaces, tabs, or a
newline-authored boundary — all reach the glued form), a whitespace-only element
collapses (`<b> </b>` → `<b></b>`), and the trim stops at content: an NBSP is content
(never trimmed), a comment is a node (the space before a boundary comment is a fragment
edge and trims; the space after it is inter-sibling and stays — render-safe under
`preserveComments` too), and inter-sibling spaces are render-significant and stay.

Prettier instead preserves an authored boundary space whenever the content fits (the
HTML/CSS inline whitespace model, which Svelte 5 deliberately broke from), collapsing any
boundary run — extra spaces, tabs, newlines — to a single kept space.

- `prettier_variant_spaces.svelte` — prettier's stable single-space boundary forms; tsv
  normalizes every case to the glued `input.svelte`.
- `unformatted_ours_spaces.svelte` — multi-space and tab-run boundary authorings; tsv
  normalizes them to `input.svelte`, prettier to the single-space forms.
- `unformatted_ours_newlines.svelte` — newline boundaries that collapse (content still
  fits): a newline around **text** is a word separator, and a **lone one-sided** newline
  around an element child is not an expansion signal (both-or-neither), so these all
  reach the glued fixed point under tsv; prettier collapses each to a kept space.
- `prettier_variant_newline_spaces.svelte` — prettier's stable output on those newline
  authorings: one kept space per authored newline side (so the one-sided case keeps a
  leading space only); tsv normalizes it to the glued `input.svelte`.
- `variant_newline_expanded.svelte` — the boundary of the trim, pinned dual-stable: a
  **both-side newline-authored** boundary around an element child keeps its layout
  meaning (`<span>⏎\t<strong>…⏎</span>` stays multiline under both formatters —
  newlines are authoring intent; only render-free space/tab runs and collapsing
  newlines are trimmed).

## Reason

Svelte-mirror whitespace: whenever tsv keeps content inline, every whitespace character
in the output is one the compiler keeps — a space belongs *between* sibling nodes, never
inside a content boundary. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).

## Related

- [inline_boundary_whitespace_multiline](../inline_boundary_whitespace_multiline_prettier_divergence/) —
  the same convergence once the content goes multiline
- [blocks/boundary_space_trim](../../blocks/boundary_space_trim_prettier_divergence/) —
  the block-section boundary counterpart
- [text_non_breaking_whitespace](../text_non_breaking_whitespace_prettier_divergence/) —
  NBSP handling in inline text
