# ws_collapsing_container_text_child_prettier_divergence

The **boundary** of `ws_collapsing_containers_prettier_divergence`. A whitespace-collapsing
container (`select` / `table` / `tbody` / `thead` / `tfoot` / `tr` / `colgroup` / `datalist` —
Svelte's `clean_nodes` `can_remove_entirely`) lays out block-style because the compiler removes an
inter-sibling whitespace-only **run** entirely. That argument covers a boundary between two
*non-text* children, where the injected line break becomes its own whitespace-only node and is
removed. It does **not** cover a boundary with a **text** child: there the break lands *inside*
that text node, where `can_remove_entirely` never applies — the run collapses to a single rendered
space instead of vanishing (`clean_nodes` removes a node only when its data is exactly `' '`).

So at a text boundary the authored form is reproduced exactly: a **glued** boundary stays glued
(breaking it would inject a rendered space), and an authored **space** is spent on the line break
rather than written twice. The container's own edges are always trimmed — the compiler strips the
first and last text node's outer whitespace regardless — so a lone text child still moves to its
own line.

Cases (in order):

1. **Lone text child** — moves to its own line; the container edges are trimmed, so no space is
   added on either side.
2. **Text glued after an element** — stays welded to `</option>`; no break at that boundary.
3. **Text glued before an element** — the mirror; the element stays welded to the text.
4. **Authored space before the text** — render-significant, and the line break carries it, so the
   text starts its line with no extra space.
5. **`<datalist>`** — the same glued case in a second container of the set.
6. **Control: two glued elements** — a whitespace-only boundary, removed entirely, so the break is
   free and the container block-styles as `ws_collapsing_containers` describes.

tsv: as above.

Prettier: keeps a container authored inline on one line — see `prettier_variant_inline.svelte`
(prettier keeps that form stable; tsv normalizes it to `input.svelte`). Given the block-style
`input.svelte` it preserves the authored line breaks except for the lone text child, which it
collapses back onto the container line as `<select> text </select>` — see
`output_prettier.svelte`.

## Reason

Render fidelity. tsv's container block-style is licensed by `can_remove_entirely`, and that
licence stops at a text node: whitespace merged into a text node renders as a space, so injecting
a break at a glued text boundary changes the compiled output (`<option>a</option>text` becomes
`<option>a</option> text`, verified against `svelte/compiler`), and re-emitting an authored
boundary space *beside* the break writes a space the next pass reads as indentation and drops —
leaving the format with no fixed point.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
