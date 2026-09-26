# lifted_run_region_trail_above_section_prettier_divergence

An instance `<script>` wrapped in `<!-- #region i -->` … `<!-- #endregion -->` written at the top
of the document, and a `<script module>` with its own `<!-- #endregion -->` trail written between
two paragraphs (`unformatted_ours_lifted_instance_first`). The canonical order prints the module
script first, so its trail ends up directly above the instance script's `#region` — and a
comment directly above a `<script>` is that script's leading comment, not the section above's
trail (prettier-plugin-svelte's precedence, which tsv matches). tsv prints that reading in one
pass: the module script, then the `#endregion` and the `#region i` leading the instance script,
which keeps its own trail.

Prettier's first pass keeps the `#endregion` against the module script
(`prettier_intermediate_lifted_instance_first.svelte`); its own second pass reads it as the
instance script's leading comment and lands on `input.svelte`.

## Reason

◆prettier_bug (non-idempotent). See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
