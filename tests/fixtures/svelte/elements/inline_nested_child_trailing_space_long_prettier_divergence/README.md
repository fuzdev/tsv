# inline_nested_child_trailing_space_long_prettier_divergence

An inline `<span>` wraps a wide inline child `<span>` (its open tag overflows print width) plus
trailing text. Under block-style both tags stay intact and the content goes on its own indented
lines (no dangle). The trailing text's boundary is **layout-keyed** — the child renders multiline,
so each spelling is a fixed point:

- **`input.svelte`** (space boundary) — the hugged form, matching the `_trailing_space` name and
  the terminal sibling `inline_wide_content_trailing_long`.
- **`variant_ownline.svelte`** (newline boundary) — **dual-stable**: both formatters keep the text
  on its own line (the layout-keyed preserve).

The prettier divergence is pinned on the **compact authoring**: `unformatted_ours_compact` (the
content on one line) normalizes to `input.svelte` under tsv, while prettier dangles the tag
delimiters into the pyramid captured by `prettier_variant_compact` (which tsv likewise converges to
`input.svelte`). So tsv lays the nested child + trailing text out block-style where prettier dangles
— the same divergence as the other inline-content fixtures, here with the trailing-space hug.

## Reason

tsv treats printWidth as a hard limit and lays the nested child + trailing text out block-style
rather than dangling. The tail boundary beside the multiline-rendering child is layout-keyed: the
space spelling hugs, the newline spelling keeps its line — the same rule the terminal sibling
`inline_wide_content_trailing_long` follows, reached here through a different render branch (the
wide child is itself multiline). See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
