# inline_nested_child_trailing_space_long_prettier_divergence

An inline `<span>` wraps a wide inline child `<span>` (its open tag overflows print width) plus
trailing text. Under block-style both tags stay intact and the content goes on its own indented
lines (no dangle). The trailing-text boundary is currently **authoring-dependent**:

- **`input.svelte`** (newline boundary) — the trailing text keeps its **own line**.
- **`variant_hug.svelte`** (space boundary) — the trailing text **hugs** the child's closing tag
  (`</span> text`).

Both forms are **dual-stable** — tsv and prettier each keep their respective form idempotent — so
this fixture documents the authoring-dependence rather than a tsv-vs-prettier disagreement on a
single input.

## Reason

Converging the two authorings (always hugging the trailing text, reflowing the newline boundary as
render-free under Svelte 5) is a deliberate **pending follow-up** — the "1b" between/terminal-text
hug-convergence. The terminal case (`inline_wide_content_trailing_long`) already hugs a space-
authored tail; this nested-child case goes through a different render branch (the wide child is
itself multiline) and is not yet converged. Until then the two authorings settle on the two distinct
stable forms above. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
