# inline_nested_child_trailing_space_long_prettier_divergence

An inline `<span>` wraps a wide inline child `<span>` (its open tag overflows print width) plus
trailing text. Under block-style both tags stay intact and the content goes on its own indented
lines (no dangle). The trailing-text boundary is currently **authoring-dependent**:

- **`input.svelte`** (space boundary) — the trailing text **hugs** the child's closing tag
  (`</span> text`). This is the fixture's canonical form, matching the `_trailing_space` name and
  the terminal sibling `inline_wide_content_trailing_long`.
- **`variant_ownline.svelte`** (newline boundary) — the trailing text keeps its **own line**.

Both forms are **dual-stable** — tsv and prettier each keep their respective form idempotent.

The prettier divergence is pinned on the **compact authoring**: `unformatted_ours_compact` (the
content on one line) normalizes to `input.svelte` under tsv, while prettier dangles the tag
delimiters into the pyramid captured by `prettier_variant_compact` (which tsv likewise converges to
`input.svelte`). So tsv lays the nested child + trailing text out block-style where prettier dangles
— the same divergence as the other inline-content fixtures, here with the trailing-space hug.

## Reason

Converging the two authorings (always hugging the trailing text, reflowing the newline boundary as
render-free under Svelte 5) is a deliberate **pending follow-up** — the between/terminal-text
hug-convergence. The terminal case (`inline_wide_content_trailing_long`) already hugs a space-
authored tail; this nested-child case goes through a different render branch (the wide child is
itself multiline) and is not yet converged, so the newline authoring (`variant_ownline`) settles on
its own distinct stable form. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
