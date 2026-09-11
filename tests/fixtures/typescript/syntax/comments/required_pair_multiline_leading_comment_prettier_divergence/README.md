# required_pair_multiline_leading_comment_prettier_divergence

The MULTI-LINE half of
[required_pair_leading_comment](../required_pair_leading_comment_prettier_divergence/): a
comment the author wrote **inside** a required paren pair stays inside it, and the pair's
operand **stays flat**.

- Input: `(/* c⏎d */ b + c + d) as T` — Prettier: `/* c⏎d */ (b + c + d) as T`
- Same at every position whose pair comes from `needsParentheses`: a binary operand, a
  member-call base, a non-null operand, an angle-bracket assertion operand, an `await`
  operand
- Ours: all of them keep the comment after the `(`, and none of them breaks the operand

**Position** is the sibling's divergence, cataloged once for both: prettier's parens are
added inside `print()` and `printComments` wraps the result, so a leading comment can only
land outside a pair emitted that way.

**The break is not a divergence at all** — both formatters keep the operand flat. The
comment prints *outside* the operand's own group, so the hard break its body carries
reaches the pair and no further. The null controls are the ones that show it with no
position question in the way: `typeof`, `!` and a sequence's own operand run put the
comment in the same place in both formatters, and there the two outputs agree byte for
byte.

Both positions are dual-stable in our formatter — prettier's output is a fixed point of
tsv too, pinned by `variant_hoisted.svelte`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
