# declaration_terminator_comment_long_prettier_divergence

The print-width boundary of
[declaration_terminator_comment](../declaration_terminator_comment_prettier_divergence/):
the tag emits the terminator-gap comment run **outside** the declaration's own group (the
run and the `;` are the tag's, not `tsv_ts`'s), and this pins that the run is nonetheless
**measured** by that group — the `=` breaks at exactly the width it would if the comment
were part of the declaration.

Two pairs, each at its own exact 100/101:

- `a` / `b` — a block comment. Flat at 100, broken at `=` at 101, the comment riding the
  value's line. Prettier breaks at the identical width, so the *layout* here agrees; only
  the comment's side of the `;` diverges.
- `c` / `d` — a `//`, whose own `hardline` carries the `;}` to the next line. The forced
  break does **not** pull the `=` open: width still decides, so 100 stays flat and 101
  breaks, exactly as the block pair does.

Prettier additionally **overflows** both `//` cases (102 and 103 columns): having moved the
comment past the tag's `}` it no longer measures it against the value at all, so the value
that should have broken stays flat. tsv holds 100 as a hard limit — see
[§Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy).

## Reason

Same divergence as the base fixture — prettier's placement is a dead document for a block
comment and rendered page text for a `//`. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [declaration_terminator_comment](../declaration_terminator_comment_prettier_divergence/) — the rule and every shape it reaches
- [declaration_long](../declaration_long/) — the declaration tag's width boundary with no comment in play
