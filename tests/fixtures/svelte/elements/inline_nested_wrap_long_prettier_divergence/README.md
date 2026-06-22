# inline_nested_wrap_long_prettier_divergence

Nested inline elements (`<strong>` wrapping `<em>`) whose combined content overflows. Both lay out
**block-style** — each tag pair stays intact, content on its own indented line(s) — composing
cleanly. The short companion (which stays inline) is `elements/inline_nested_deep`.

Prettier instead emits a **double dangle pyramid** (`<strong⏎\t><em⏎\t\t>…</em⏎\t></strong⏎>`); tsv
keeps every tag intact. The `unformatted_ours_compact` (single-line authoring) and
`prettier_variant_compact` (prettier's stable double-pyramid) both normalize to `input.svelte` under
tsv in one pass.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
