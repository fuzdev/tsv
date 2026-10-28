# Assignment value's left-side shell comment, with a line comment in the chain

The [rhs_left_side_shell_comment](../rhs_left_side_shell_comment_prettier_divergence/) rule
where the value's chain also holds a line comment of its own
(`x = (// c1⏎x.a() // c2⏎)[x];`). The value's printer hoists `// c1` out of the stripped
shell, so its doc opens with that run, and the value hangs under the operator, as it does for
a line comment in the operator's own gap — at an assignment, a declarator and a class field
alike. `unformatted_ours_inner_shell` carries the authoring.

- **tsv**: `x =⏎\t// c1⏎\tx⏎\t\t.a() // c2⏎\t\t[x];`. The chain breaks around its own
  comment as it does with nothing before it (`y = x⏎\t.a() // c⏎\t[x];`), the cataloged
  [group_interior_member_line_comment](../../calls/chained/group_interior_member_line_comment_prettier_divergence/)
  layout.
- **prettier**: hangs the value the same way and keeps the chain's head on one line
  (`x =⏎\t// c1⏎\tx.a() // c2⏎\t[x];`, `output_prettier.svelte`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
