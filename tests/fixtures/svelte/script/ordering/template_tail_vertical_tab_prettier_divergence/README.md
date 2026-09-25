# template_tail_vertical_tab_prettier_divergence

Template text ending in a vertical tab, authored ahead of the `<script>`
(`unformatted_ours_before_script.svelte`). U+000B is not whitespace to Svelte's compiler
(`clean_nodes` trims only `[ \t\r\n]`) and not HTML whitespace either, so it renders; it is
ECMAScript `WhiteSpace`, so Svelte's `parse` trims it with `trimEnd()` once the reorder leaves
the text at the end of the document
([template_tail_nbsp](../template_tail_nbsp_prettier_divergence/)). tsv spells it `&#xB;`.

Prettier keeps the raw character on its first pass
(`prettier_intermediate_to_variant_before_script.svelte`) and loses it on its second, landing on
`variant_vertical_tab_dropped.svelte` — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
