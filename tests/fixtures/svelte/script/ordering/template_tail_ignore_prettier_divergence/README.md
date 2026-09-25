# template_tail_ignore_prettier_divergence

A `<!-- prettier-ignore -->`-frozen text ending in a non-breaking space, authored ahead of the
`<script>` (`unformatted_ours_before_script.svelte`). The freeze keeps the node's bytes, but
the reorder — tsv's own move, not the author's — leaves it at the end of the document, where
Svelte's `parse` would trim the U+00A0 with `trimEnd()`
([template_tail_nbsp](../template_tail_nbsp_prettier_divergence/)). tsv completes the move
losslessly: the frozen node's last character is spelled `&#xA0;`, the one respelling a freeze
admits. A frozen node the reorder leaves where it was keeps its bytes exactly.

Prettier keeps the raw character on its first pass
(`prettier_intermediate_to_variant_before_script.svelte`) and loses it on its second, landing on
`variant_nbsp_dropped.svelte` — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
