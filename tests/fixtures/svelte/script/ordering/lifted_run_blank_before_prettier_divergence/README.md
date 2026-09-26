# lifted_run_blank_before_prettier_divergence

A `<script>` with a blank line written ABOVE it and only a newline or a space below it, between
two paragraphs (`unformatted_ours_lifted_block_newline_after`,
`unformatted_ours_lifted_block_space_after`) and between text and a paragraph
(`unformatted_ours_lifted_text_block_newline_after`,
`unformatted_ours_lifted_text_block_space_after`). The script goes to the top and the neighbours
join as if it had never been there, so the authored blank line stays between them — once. A
blank line on both sides is still one blank line (`unformatted_lifted_block_blank_lines`, where
prettier agrees).

Prettier reads only the whitespace below the section, so the blank line above it is dropped:
`variant_block_blank_dropped.svelte` and `variant_text_block_blank_dropped.svelte`.

## Reason

◆stable_quirk. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
