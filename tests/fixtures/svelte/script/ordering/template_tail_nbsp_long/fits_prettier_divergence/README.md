# fits_prettier_divergence

The width boundary of the template-tail reference
([template_tail_nbsp](../../template_tail_nbsp_prettier_divergence/)): a line of text ending in a
non-breaking space, authored ahead of the `<script>`, which the reorder leaves at the end of the
document, where tsv spells its last character as `&#xA0;`.

The reference is part of the text the line is measured with. Spelled `&#xA0;`, the line is
**exactly 100** chars, so it stays on one line. Raw, the same line is 95
(`unformatted_ours_before_script.svelte`) — the width a layout that respelled after measuring
would have seen. The sibling [breaks](../breaks_prettier_divergence/) is the 101 case.

`unformatted_wrapped.svelte` holds the respelled text wrapped before its last word; since the
line fits, both formatters rejoin it.

Prettier keeps the raw character on its first pass
(`prettier_intermediate_to_variant_before_script.svelte`) and loses it on its second, landing on
`variant_nbsp_dropped.svelte` — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
