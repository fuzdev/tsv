# left_spine_paren_prettier_ignore_required_pair_angle_prettier_divergence

The trailing-`>` join of a frozen `new DD<T>`, every cell of it but `+`. The slice is the
node's own bytes and ends at the type arguments' `>`, so the question at a binary position
is which tail JOINS that `>` — and the answer is a REJECTION table over the three parsers
that grade the output, never a precedence question. The pair goes around the slice wherever
the bare spelling is not a form all three read as the input meant: a silent rebind, or a
rejection at any one of them.

| tail | tsc | acorn-typescript | tsv | bare reading |
| --- | --- | --- | --- | --- |
| `-` | accepts | accepts | accepts | rebinds at all three: `new DD<T> - 1` is `((new DD) < T) > -1` |
| `<` | rejects — `'>' expected` | accepts | accepts | same tree at acorn-typescript and tsv |
| `>=` | rejects — `Expression expected` | accepts | accepts | same tree at acorn-typescript and tsv |
| `<<` | accepts | rejects | accepts | same tree at tsc and tsv |
| `>` | rejects | rejects | rejects | no reading at all |
| `>>` | rejects | rejects | rejects | no reading at all |
| `>>>` | rejects | rejects | rejects | no reading at all |
| `<=` | accepts | accepts | accepts | same tree everywhere — the control, and the one cell that takes NO pair |

```ts
const ee = (
	// prettier-ignore
	new DD<T>
) < 1;
```

`unformatted_ours_angle_shell.svelte` is that parenthesized authoring, one twin per cell, and
`input.svelte` is the form tsv converges to. The `<=` row is the absence pin beside them: `<=`
opens no continuation at any parser, so the parse backtracks to the type arguments, the bare
form is the same tree, and tsv prints no pair.

**`<<` is the one join no reparse-based gate can see.** tsc reads the bare form back as the
same tree and so does tsv, so prettier's own output parses at prettier's parser, tsv's reparse
of that output agrees, and nothing anywhere errors — only acorn-typescript, whose wire tsv is
a drop-in for, rejects it, which is why the cell is pinned here by hand. `<` and `>=` are the
half-visible pair: a reparse by tsv is blind to them too, and what exposes them is prettier
failing to re-read its own first pass (below). Only `>` / `>>` / `>>>` are loud everywhere.

Both tools hold `input.svelte`, so the divergence here is entirely what prettier does with the
parenthesized spelling. The join's remaining cell, `+`, lives in the sibling
[left_spine_paren_prettier_ignore_required_pair_prettier_divergence](../left_spine_paren_prettier_ignore_required_pair_prettier_divergence/),
whose own prettier chain is pinned and so cannot hold a cell that truncates it — which is
every cell here.

## Why tsv differs

Prettier's ignore path emits the frozen slice without re-asking `needsParens`, so its first
pass drops every pair — `new DD<T> - 1`, `new DD<T> < 1`, `new DD<T> >>> 1` — and then
**cannot re-read its own output**: pass 2 throws `'>' expected` at the `<` cell, the first of
the five its parser rejects. A chain that ends in a throw is not a fixed point, and no marker
can pin it, which is why this fixture documents the divergence by README alone and carries no
prettier-anchored claim file. tsv re-asks the question at the frozen seam and keeps the pair,
so its output means what the input meant and reparses everywhere.

## Reason

◆comment_preservation ◆prettier_bug — sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On delimiter-owned value heads, and on sequence operands*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
