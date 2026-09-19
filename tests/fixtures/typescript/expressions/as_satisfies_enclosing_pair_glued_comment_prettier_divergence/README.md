# as_satisfies_enclosing_pair_glued_comment_prettier_divergence

A block comment glued to an `as` / `satisfies` cast's operand, where the cast takes a
paren pair of its own from its position (a member base, a callee, a logical operand, a
ternary test, a non-null operand).

**tsv**: the comment is bound to the token it is glued to, so the pair goes outside it:

```ts
const a = (/* c1 */ x as A).b;
```

**Prettier**: applies leading comments around whatever the printer returned, parens
included, so the pair lands between the comment and its token:

```ts
const a = /* c1 */ (x as A).b;
```

## Reason

The ownership rule — every glued block comment is printed by the node its token begins,
so no pair synthesized around an enclosing expression can land between the two. `const f`
is the boundary: with no pair to add, both formatters agree.

`unformatted_ours_paren_shell.svelte` authors each operand inside a redundant grouping
shell with the comment glued to the inner `(` (`((/* c1 */ (x)) as A).b`). Both shells
strip, and the comment leads the operand inline (prettier takes two passes from there: its
first lands on tsv's form, its second hoists — `const g`, left bare in the variant, is the
authoring it hoists at once) — the cast's operand shell is kept only
for a comment the bare form cannot place (a `//`, a multi-line block, an own-line block:
[as_satisfies_operand_line_comment](../as_satisfies_operand_line_comment_prettier_divergence/)),
never for one that glues to the operand once the shell is gone. The comment-only
positions, where the two formatters agree, are
[as_satisfies_operand_shell_glued_block_comment](../as_satisfies_operand_shell_glued_block_comment/).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
