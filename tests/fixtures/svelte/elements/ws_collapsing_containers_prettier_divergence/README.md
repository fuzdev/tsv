# ws_collapsing_containers_prettier_divergence

Svelte's compiler removes an inter-sibling whitespace-only run **entirely** — rather than
collapsing it to a single rendered space — when the parent is one of the whitespace-collapsing
containers: `select`, `table`, `tbody`, `thead`, `tfoot`, `tr`, `colgroup`, `datalist`, and SVG
elements outside a `<text>` element (Svelte's `clean_nodes` `can_remove_entirely`). This is the
**exact** Svelte set, verified element-by-element against the compiler — it is deliberately a
*subset* of what the HTML spec collapses (Svelte's source carries a `// TODO others?`), so
`optgroup` / `ul` / `ol` / `menu` / `dl` / `fieldset` are **not** included: Svelte keeps their
inter-sibling space, and tsv must too (tsv is a drop-in for Svelte's compiler, not for raw HTML).

Because inter-sibling whitespace never renders in these containers, tsv lays them out
**block-style** — each child on its own indented line, no rendered space between siblings —
matching the compiled render, the same stance tsv takes at every other render-free boundary
(element/component content, block bodies). Prettier applies the HTML/CSS inline whitespace model
uniformly and keeps a container authored inline on one line.

Cases:

1. `<select>` with `<option>` children — laid out block-style.
2. `<table>` / `<tbody>` / `<tr>` with `<td>` children — each structural level on its own line
   (leaf cell content stays inline).
3. **Control** `<div><span>a</span> <b>b</b></div>` — an ordinary parent, where the inter-sibling
   space is render-significant and therefore **preserved inline** by both formatters. This pins
   that the rule is specific to `can_remove_entirely` parents and must not leak to ordinary
   elements.

tsv: a whitespace-collapsing container lays out block-style. Prettier: keeps an inline authoring
inline — see `prettier_variant_inline.svelte` (prettier keeps that single-line form stable; tsv
normalizes it to the block-style `input.svelte`).

SVG (`<svg><rect/> <rect/></svg>`, and its `<text>` exception which *preserves*) is the same rule
but namespace-based rather than name-based, and is a follow-up.

## Reason

Design choice, render-free under Svelte 5. In a `can_remove_entirely` container the compiler
deletes inter-sibling whitespace outright (verified against `svelte/compiler` `clean_nodes` and
by compile-diff — the block-style and inline forms compile identically), so tsv treats the
boundary as render-free and lays the container out block-style, uniformly with every other
render-free fragment boundary. This extends the whitespace-trim stance to the one inter-sibling
position that is also render-free.
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
