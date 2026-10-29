# Template interpolation, comment in a stripped shell at the value's edge

A comment inside a grouping shell the printer strips, at the head of a `${…}` value
(`` `x${(/* c1 */ bbb)`⏎`}y` ``) or at the end of a binary's right operand
(`${() => … + (jjj /* c4 */)}`). Stripping the
shell prints the comment at the value's edge, right behind the `${` or ahead of the `}` —
where the shell-free spelling writes it — so once the value breaks, the interpolation breaks
at its `${` and `}` around it, as it does for any comment there
([interpolation_comment_expression_kinds](../interpolation_comment_expression_kinds/)).

- **tsv**: reads the comment at the value's printed edge, so the authored form
  (`unformatted_ours_shell`) reaches `input.svelte` in one pass — in a `<script>`, a nested
  `${}` (`c6`), behind a non-null tag (`c7`), a template island (`c5`) and an attribute
  expression (`c8`) alike.
- **prettier**: lands on the same form in **two passes**. Its first attaches the comment
  inside the value, so `hasComment(node)` misses it and the interpolation hugs a value that
  breaks (`` `x${/* c1 */ bbb`⏎`}y` ``, `prettier_intermediate_shell.svelte`); the reparse
  finds the comment at the interpolation's edge and breaks around it.

Not a placement difference — the fixed point is shared. A comment a pair the printer KEEPS
holds (`${(/* c */ a + b).m(…)}`, the member object's parens) stays behind that `(` and
occupies nothing at the edge
([interpolation_shell_edge_comment_held](../interpolation_shell_edge_comment_held_prettier_divergence/)). The width-driven break is
[interpolation_shell_edge_comment_long](../interpolation_shell_edge_comment_long_prettier_divergence/).

See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
