# lifted_run_glued_entity_long_prettier_divergence

The width boundary of the glued-seam reference
([lifted_run_glued_entity](../lifted_run_glued_entity_prettier_divergence/)): a line of text whose
last word ends in an open character reference, written ahead of a `<script>` glued to the text
that completes it (`… x&amp<script>…</script>;y`). tsv joins the two and spells the first
character after the seam as a reference, `x&amp&#x3B;y`.

The reference is part of the text the line is measured with. Respelled, the first text would be
101 chars on one line, so its last word wraps (`unformatted_ours_breaks.svelte`), and the second is
exactly 100, so it stays on one line (`unformatted_ours_fits.svelte`). Joined as written, `x&amp;y`,
the same texts are 96 and 95 — the widths a layout that respelled after measuring would have seen.

Prettier joins the texts as written, landing on `x&amp;y` (`variant_breaks.svelte`,
`variant_fits.svelte`), which renders `x&y` where the source renders `x&;y` — a render change its
own next pass keeps.

## Reason

◆prettier_bug. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and [conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index).
