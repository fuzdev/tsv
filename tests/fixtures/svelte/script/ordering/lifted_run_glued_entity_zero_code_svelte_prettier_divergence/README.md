# lifted_run_glued_entity_zero_code_svelte_prettier_divergence

The zero-code edge of the glued-seam reference
([lifted_run_glued_entity](../lifted_run_glued_entity_prettier_divergence/)): a hoisted section
glued between a text ending in `&#0` and a text that continues it with a digit
(`x&#0<script>…</script>0y`, `unformatted_ours_digit.svelte`) or a `;`
(`x&#0<style>…</style>;y`, `unformatted_ours_semicolon.svelte`).

**This case is not a prettier bug.** Prettier joins the texts as written, `x&#00y` and `x&#0;y`
(`variant_digit.svelte`, `variant_semicolon.svelte`), and that renders the same page: Svelte's
decoder leaves a zero-code reference as literal text (its `if (!code) return match` guard), so
the two texts read apart and the joined text all render `x&#00y` / `x&#0;y`.

tsv respells anyway (`x&#0&#x30;y`, `x&#0&#x3B;y`) because it asks the question of its own
decoder, which follows the spec and decodes a zero code to NUL: by that reading the join turns
`&#0` + `0` into one reference, so the joined text would decode differently from the two apart.
Its decoder reads a reference at least as far as Svelte's does, so the test can only respell
where Svelte's would not, never miss a seam Svelte's would complete. Here the respell is
conservative: the reference decodes to the character it replaces, and the page is unchanged.

The input also carries the parse divergence the zero code brings with it: tsv decodes `&#0` to
NUL where Svelte keeps it literal (`expected_ours.json`, `expected_svelte.json`).

## Reason

◆design_choice. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and, for the decoding, [conformance_svelte.md §Entity Decoding Corrections](../../../../../../docs/conformance_svelte.md#entity-decoding-corrections).
