# rhs_left_side_shell_jsdoc_cast_prettier_divergence

An own-line JSDoc cast at the head of an assignment value's member chain hangs the value under
the operator — the cast prints a hardline between its comment and its `(`, and a hardline with
no hang leaves that `(` at the binding's own indent. Both formatters keep `input.svelte`.

The cast can also be reached through a grouping shell the chain prints **mid-chain**
(`x = (⏎/** @type {T} */⏎(f).b).c(zz)`): the shell's own run prints after the chain's head, but
the cast's comment is owned by the cast, which prints it ahead of the head, so the value's doc
still opens with the comment and its line. tsv hangs that spelling too and lands on
`input.svelte` in one pass (`unformatted_ours_mid_chain_shell.svelte`):

```ts
x =
	/** @type {T} */
	(f).b.c(zz);
```

Prettier settles elsewhere: its first pass prints the comment after the cast's `(f)`
(`x = (f)/** @type {T} */⏎.b⏎\t.c(zz);`), so the second reads no cast, drops the pair and lands
the comment inside the chain (`x = f./** @type {T} */ b.c(zz);`), pinned by
`audit_signature_mid_chain_shell.txt`.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
