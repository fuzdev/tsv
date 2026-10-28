# Left-spine shell comment below a non-null: convergence

The [left_spine_shell_comment_nested_pair](../../../syntax/comments/left_spine_shell_comment_nested_pair_prettier_divergence/)
rule with a non-null on the spine: a comment in a discarded grouping pair below a `!` —
standalone (`x = ((/*c*/ (a).b())! + 1).c`) or a link of the chain above it
(`x = ((/*c*/ (a).b())!() + 1).c`) — under a pair the spine prints. The comment leads the
whole value, ahead of that pair; under a prefix unary it leads the operand inside the pair
the comment earns there (`x = typeof (/*c*/ a.b.c!())`), and a line comment takes a line of
its own ahead of the pair (`// c⏎(a.f()! as T)();`).

- **tsv**: one pass, `input.svelte`. The non-null prints its operand's run like every other
  site on the spine, so the run is printed once, by the site that claims it.
- **prettier**: reaches the same form on its second pass; its first leaves the comment
  inside the pair (`x = (/*c*/ a.b()! + 1).c`, `prettier_intermediate_nested.svelte`).

`unformatted_ours_nested` carries the authoring.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Left-spine shell comment through nested discarded pairs, convergence) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
