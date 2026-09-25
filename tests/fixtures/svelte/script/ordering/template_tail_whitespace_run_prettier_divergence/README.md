# template_tail_whitespace_run_prettier_divergence

Template text ending in a **run** of characters Svelte's compiler keeps and JavaScript's
`trimEnd()` removes — U+3000 IDEOGRAPHIC SPACE, then a non-breaking space — authored ahead of the
`<script>` (`unformatted_ours_before_script.svelte`). The reorder leaves the text at the end of
the document (see [template_tail_nbsp](../template_tail_nbsp_prettier_divergence/)).

Only the **last** character is spelled as a reference, `a　&#xA0;`: `trimEnd()` stops at the
first character it does not remove, and the `;` is that character, so the U+3000 behind it is no
longer at the end and stays raw.

Prettier keeps the run on its first pass
(`prettier_intermediate_to_variant_before_script.svelte`) and trims both characters on its
second, landing on `variant_run_dropped.svelte` — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
