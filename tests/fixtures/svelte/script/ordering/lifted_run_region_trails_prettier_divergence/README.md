# lifted_run_region_trails_prettier_divergence

Two hoisted sections, each wrapped in a `#region` … `#endregion` pair — a `<script>` whose
`<!-- #endregion -->` is its region-end trail, and a `<style>` led by `<!-- #region b -->` —
written between template nodes: on lines of their own between three words
(`unformatted_lifted_newlines`), glued between them (`unformatted_ours_lifted_glued`), and the
script between two paragraphs with a blank line above it (`unformatted_ours_lifted_blank_before`).
Each run travels to its canonical position with its comments — the `#region` above, the
`#endregion` below — and the neighbours join as if it had never been there: `x y z`, `xyz`, and
the blank line kept between the paragraphs.

Prettier agrees on the newline spelling. Glued, it drops the template's last node
(`variant_last_node_dropped.svelte`); with the blank line written above the script run and only a
newline below it, it drops the blank (`variant_blank_dropped.svelte`).

## Reason

◆content_preservation (the dropped node) and ◆stable_quirk (the blank line). See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
