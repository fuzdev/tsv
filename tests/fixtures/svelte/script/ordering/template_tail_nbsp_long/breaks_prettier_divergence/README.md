# breaks_prettier_divergence

The width boundary of the template-tail reference
([template_tail_nbsp](../../template_tail_nbsp_prettier_divergence/)), one char past
[fits](../fits_prettier_divergence/): the text's last word, `abcde` followed by a non-breaking
space, spelled `abcde&#xA0;`, would end the line at **101** chars, so it wraps onto its own line.

Raw, the same line is 96 (`unformatted_ours_before_script.svelte`) and would fit — so the
reference has to be in the text the fill measures, not substituted after it: a line laid out raw
and respelled afterwards comes out 101 wide, and the next pass wraps it.

`unformatted_one_line.svelte` holds the respelled text on one 101-char line; both formatters wrap
it.

Prettier keeps the raw character, on one line, on its first pass
(`prettier_intermediate_to_variant_before_script.svelte`) and loses it on its second, landing on
`variant_nbsp_dropped.svelte` — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
