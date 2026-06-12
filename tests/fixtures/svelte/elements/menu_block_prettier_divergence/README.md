# menu_block_prettier_divergence

prettier-plugin-svelte's `blockElements` list includes `ol` and `ul` but omits `menu`. The HTML spec treats `<menu>` identically to `<ul>`.

tsv: treats `<menu>` as block element (spec-compliant)
Prettier: treats `<menu>` as inline (hugs content)

## Reason

The HTML spec defines `menu` with `display: block` and the same CSS rules as `ul`/`ol`. The prose describes it as "simply a semantic alternative to `ul`." This is a bug in prettier-plugin-svelte's `blockElements` array.
