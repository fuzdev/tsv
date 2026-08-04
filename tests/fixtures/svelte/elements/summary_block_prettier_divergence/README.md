# summary_block_prettier_divergence

prettier-plugin-svelte's `blockElements` list includes `details` and `li` but omits `summary`. The HTML spec gives `<summary>` the same UA display as both.

tsv: treats `<summary>` as block element (spec-compliant)
Prettier: treats `<summary>` as inline (trailing text hugs the closing tag)

## Reason

**Spec violation.** The HTML spec's rendering section defines `details, summary { display: block; }`, and gives the disclosure triangle its own rule on top — `details > summary:first-of-type { display: list-item; ... }`, the same `display` as `li`. prettier-plugin-svelte's `blockElements` list carries `details` and `li` but omits `summary`, so it classifies two elements with identical UA display in opposite ways; tsv includes it.

The plugin is also the outlier within its own project: prettier's HTML printer derives element display from the UA stylesheet rather than a hand-maintained list, and formats `<summary>` as block exactly as tsv does. The divergence is against the Svelte plugin, not against prettier.

The visible effect is on a `<summary>`'s following sibling: as an inline element the space boundary collapses into the surrounding fill and the sibling hugs the closing tag (`</summary> text2 text3`), where as a block element `<summary>` takes its own line and the sibling keeps its own.

This only manifests when compact input is formatted — both formatters preserve the block form if given it directly.

See [conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
