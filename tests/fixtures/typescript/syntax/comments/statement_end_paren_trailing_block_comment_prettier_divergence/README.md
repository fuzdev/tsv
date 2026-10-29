# statement_end_paren_trailing_block_comment_prettier_divergence

A block comment written last inside a grouping pair whose `)` ends the value of an
expression statement or of a variable declaration's last declarator (statement clause
bodies included — `if`, the loops, a label): `x + (y /* c */);`, `v = a + (b /* c */);`, `x = a + (q + b /* c */);` —
the stripped shell of a binary chain's last operand, a pair the operator requires around
that operand, and a value shell with another comment behind it.

**tsv**: the statement's terminator gap opens where the value's printed content ends — down
its right spine, through a binary's last operand as the chain prints it — so the trailing
run is the terminator's to emit, and a same-line block trails the statement past its `;`
in **one** pass, from every authoring:

```ts
x + y; /* c */
x = a + (q + b); /* c */
```

The authorings are the plain shell (`unformatted_ours_parens`), its ASI spelling with no
`;` (`unformatted_ours_parens_asi`, which formats as its `;` twin), and redundant nested
shells (`unformatted_ours_nested`). A comment written after the shell keeps its place
behind the one inside it (`y = (b /* c */) /* d */;` → `y = b; /* c */ /* d */`, a `//`
behind them too): one emitter prints the whole gap in source order, so a block deferred
out of the shell cannot land behind one printed after the `;`. A multi-line block trails
the same way, and the chain lays out as its comment-free twin, flat.

Some cells hold the comment where it is, in both formatters: a non-last declarator, whose
gap ends at its comma (`let s = a + b /* c */,`), and a binary inside the pair a declarator
position adds around an assignment value (`const k = x = y + (b /* c */);` →
`const k = (x = y + b /* c */);`) or an arrow adds around its conditional body
(`x = () => (c ? d : a + b /* c */);`). A value a format-ignore directive freezes prints the
region verbatim, once.

**Prettier**: reaches the same fixed point, but moves the comment out one pair per pass —
its first pass leaves the block before the `;` (`x + y /* c */;`, or after the kept pair,
`x = a + (q + b) /* c */;` — the `prettier_intermediate_parens` file), and a nested shell
takes a third pass (`audit_signature_nested.txt`). The divergence is the pass count, not
the destination.

This is the binary-chain twin of
[value_paren_trailing_block_comment](../value_paren_trailing_block_comment_prettier_divergence/),
where the shell is the value's own.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
