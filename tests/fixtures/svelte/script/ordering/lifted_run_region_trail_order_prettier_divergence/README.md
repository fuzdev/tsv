# lifted_run_region_trail_order_prettier_divergence

A `<style>` wrapped in `<!-- #region s -->` … `<!-- #endregion -->` written at the top of the
document, a `<script module>` with its own `<!-- #endregion -->` trail written between two
paragraphs, and a comment ending the template (`unformatted_ours_lifted_style_first`). Every
comment prints with the section it was written with: the module script with its trail at the
top, the paragraphs joined as if the script had never stood between them, and the template's
last comment above the `<style>`'s region, which keeps both of its markers.

Prettier prints the same order, but its first pass leaves the template's last comment glued to
the paragraph above it (`prettier_intermediate_lifted_style_first.svelte`); its second pass reads
that comment as the `<style>`'s and gives it the section blank — the form tsv prints in one pass.

## Reason

◆prettier_bug (non-idempotent). See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
