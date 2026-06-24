# menu_block_prettier_divergence

prettier-plugin-svelte's `blockElements` list includes `ol` and `ul` but omits `menu`. The HTML spec treats `<menu>` identically to `<ul>`.

tsv: treats `<menu>` as block element (spec-compliant)
Prettier: treats `<menu>` as inline (hugs content)

## Reason

**Spec violation.** The HTML spec defines `menu` with `display: block` and the same CSS rules as `ul`/`ol` (`padding-inline-start: 40px`, `counter-reset: list-item`), and the prose describes it as "simply a semantic alternative to `ul`." prettier-plugin-svelte's `blockElements` list omits `menu`, so it formats against the spec; tsv includes it.

See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
