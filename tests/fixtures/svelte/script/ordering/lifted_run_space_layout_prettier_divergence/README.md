# lifted_run_space_layout_prettier_divergence

A hoisted section written between two expression tags (`{a} <script>…</script> {b}`,
`unformatted_ours_lifted_expr`) or two components (`<A /> <script module>…</script> <B />`,
`unformatted_ours_lifted_component`) with a space on each side. The section goes to its
canonical position and the neighbours join as if it had never been there: `{a} {b}`, `<A /> <B />`,
the space-spelled pair tsv prints wherever it is authored.

`output_prettier.svelte` is prettier's output from `input.svelte`: it splits each space-spelled
pair onto two lines, as it does for the same pair with no section in it — the root
`{expr1} {expr2}` divergence of
[inline_tag_pair_space](../../../elements/inline_tag_pair_space_prettier_divergence/). Both variants
land there too. The join itself adds nothing: the layout is the one the pair takes without the
section.

## Reason

◆design_choice. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
