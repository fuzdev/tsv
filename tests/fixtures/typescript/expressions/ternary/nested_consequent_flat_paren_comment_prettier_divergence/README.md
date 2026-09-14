# Flat nested-consequent comment, outside the pair the layout mints

A comment in a ternary's **`?`→consequent gap** ahead of a **nested conditional that fits
flat**. The flat layout mints a pair around the nested conditional (`a ? (aaa ? bbb : ccc) : c`,
prettier's `ifBreak("", "(")`), and the two formatters part on which side of that pair the
gap's run lands.

```ts
// tsv (the run ahead of the pair)                 // prettier (the run inside it)
const x1 = a ? /* c */ (aaa ? bbb : ccc) : c;      const x1 = a ? (/* c */ aaa ? bbb : ccc) : c;
```

tsv prints the gap's run **ahead of the pair**, and a block the author glued to the nested
TEST — which that test owns (`Comment::owned_by_node`) — **inside** it: two authorings, two
fixed points, whichever side the author wrote (`x1` / `x2`). Prettier's `printTernary` places
the `ifBreak` paren ahead of `print(consequent)`, whose comments ride inside, so it moves the
run in and holds one form. Both are lossless.

## Why tsv differs

The pair is the *layout's*, and tsv's rule for a run at a pair the position adds is one rule
at every such pair: the run leads the branch, the pair goes around the branch's own doc. At a
**clarity** pair (`x4`, `a ? /* c */ (b ?? c) : d`) prettier answers the same way — its
`needsParens` pair goes inside `printComments`, so the run sits outside — and it is prettier
that parts between its two pairs, not tsv. The glued-inside authoring is the ternary operand
leading-comment rule one entry over
([test_paren_leading_comment](../test_paren_leading_comment_prettier_divergence/)), where
prettier collapses the other way, to the outside.

## Expected behavior

- **tsv**: `input.svelte` is a fixed point; `unformatted_ours_compact` normalizes to it.
- **prettier**: rewrites the outside authorings (`x1`, the `x3` run) to the inside form
  (`output_prettier.svelte`), which both formatters then hold stable.

## Reason

◆comment_preservation — sanctioned in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(*Flat nested-consequent comment*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
