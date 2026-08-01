# inline_nested_child_trailing_space_long_prettier_divergence

An inline `<span>` wraps a wide inline child `<span>` (its open tag overflows print width) plus
trailing text. Under block-style both tags stay intact and the content goes on its own indented
lines (no dangle). The trailing text **hugs** the child's closing tag (`</span> text`) whichever way
its boundary is authored:

- **`input.svelte`** (space boundary) — the canonical form, matching the `_trailing_space` name and
  the terminal sibling `inline_wide_content_trailing_long`.
- **`prettier_variant_ownline.svelte`** (newline boundary) — prettier keeps the text on its own line;
  tsv converges it to `input.svelte`, since the boundary is render-free and so carries no authoring
  signal to preserve.

The prettier divergence is pinned on the **compact authoring**: `unformatted_ours_compact` (the
content on one line) normalizes to `input.svelte` under tsv, while prettier dangles the tag
delimiters into the pyramid captured by `prettier_variant_compact` (which tsv likewise converges to
`input.svelte`). So tsv lays the nested child + trailing text out block-style where prettier dangles
— the same divergence as the other inline-content fixtures, here with the trailing-space hug.

## Reason

tsv treats printWidth as a hard limit and lays the nested child + trailing text out block-style
rather than dangling. The tail's placement is **not** authoring-dependent: the boundary between a
closing tag and a terminal text sibling is render-free under Svelte 5, so both authorings converge on
the hug — the same rule the terminal sibling `inline_wide_content_trailing_long` follows, reached
here through a different render branch (the wide child is itself multiline). See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
