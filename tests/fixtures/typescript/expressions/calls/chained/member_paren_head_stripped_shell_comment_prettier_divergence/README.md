# A comment in a stripped shell at the end of a paren chain head

The stripped-shell sibling of
[member_paren_head_broken_trailing_comment](../member_paren_head_broken_trailing_comment_prettier_divergence/):
a member whose object is a parenthesized head ending in a grouping pair the printer strips —
a binary's last operand, reached also through an arrow's body, a conditional's alternate, or an
assignment's or a `yield`'s operand — with a comment inside that pair, and often one more after
the head's `)`:

```ts
v = (a + (b // c1
)) /* c2 */.h();
```

- **tsv**: the gap before the member opens where the head's printed content ends, so it takes
  the shell's comment and prints it after the `)`, exactly where a comment written directly
  before that `)` goes, with each comment written after the `)` following it in authored
  order (`(a + b) // c1⏎\t/* c2 */ .h()`). `input.svelte` is that placement written
  directly — the comment after the `)`, no shell — and the `unformatted_ours_authored`
  variant writes each comment inside a stripped shell: both comment kinds after the `)`
  (`c1`–`c9`), each link with nothing after it (`c10`–`c13`), an own-line `//`, a block and a
  multi-line block in the shell (`c14`–`c19`), a logical chain, nested shells and the right
  spine's descents (`c20`–`c33`), a head whose binary breaks after a multi-line operand, laid
  out as its comment-free twin (`c34`), the restricted productions — a called member at `return`
  among them (`c51`, `c52`) — a declarator, a call argument and an `if` test (`c39`–`c52`), a
  callback (`c53`, `c54`), and the template hosts — attributes, an expression tag and an `{#if}`
  head (`c55`–`c60`). A pair the printer KEEPS holds a comment in its own gap and ends the walk:
  one the operator requires (`c35`) and a unary's (`c37`), with the comment after the `)` (`c36`,
  `c38`) staying where it was.
- **prettier**: its first pass keeps the comment inside the pair and opens it
  (`(⏎\ta +⏎\tb // c1⏎) /* c2 */`); its next pass carries it past the `)` to tsv's
  placement (`audit_signature_authored.txt`), and from there its usual relocations of the
  parenthesized head apply: an own-line comment after the `)` is hoisted above the statement,
  after the `=`, or inside `return (` / `if (`, and the head's parens expand
  (`output_prettier.svelte`, `audit_signature.txt`).

What the fixture pins is one placement for both authorings. Left to the binary, the
shell's comment rode a `line_suffix` past the `)` to wherever the line ended: behind a block
the member gap printed first (`(a + b) /* c2 */ // c1`, the pair reordered), splitting an
authored `/* c8 */ // c9` pair, past the member to the statement's end (`(a + b).h; // c10`)
or into an `if` body, to a template value's end, and out of step with the hanging pair of
`return` / `throw` / `yield`, whose reading of the gap could not see it (a second pass to
settle the pair).

Reason: Comment relocation. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
