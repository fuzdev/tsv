# Template interpolation, shell comment held away from the value's edge

The counterpart of
[interpolation_shell_edge_comment](../interpolation_shell_edge_comment_prettier_divergence/): a
comment in a stripped shell on the value's left spine that does NOT print at the `${` edge, so
the interpolation hugs its value as it does with no comment.

- `c1` — a `//` in a shell the member chain prints MID-CHAIN (`${(⏎// c1⏎bbb.ccc)⏎.ddd(eee)}`):
  the chain keeps it at the member it was written before, inside the value.
- `c2` — a comment the leaf OWNS, inside a pair the printer keeps (the member object's parens
  around a conditional, `${(/* c2 */ ggg ? hhh : iii).jjj(…)}`): it prints from the leaf, behind
  that `(`.

- **tsv**: hugs both, in one pass from the authored form (`unformatted_ours_shell`).
- **prettier**: moves both comments — the `//` past `.ccc` and the block out of the required
  pair, the cataloged relocations — and on its second pass wraps the interpolation around the
  block it moved to the edge (`output_prettier.svelte`, the chain pinned by
  `audit_signature.txt`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
