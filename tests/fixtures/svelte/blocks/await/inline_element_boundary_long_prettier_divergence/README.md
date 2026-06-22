# inline_element_boundary_long_prettier_divergence

An `{#await}` block inside an inline element at the 100/101 print-width boundary. tsv lays the
wrapping content out **block-style** — both tags stay intact and the content moves to its own
indented line, collapsing to `<tag>{#await …}…{/await}</tag>` when it fits. Prettier dangles the
delimiters instead. Content-boundary whitespace is render-free under Svelte 5.

The `unformatted_ours_*` variant is a compact authoring that tsv normalizes to the block-style input;
prettier does not (it dangles), so it carries the divergence.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
