# rhs_left_side_shell_indentable_block_prettier_divergence

A `//` glued to the `(` of an assignment value's leftmost shell, with an **indentable** block
below it. The member chain prints that shell mid-chain, so the `//` trails the chain's head;
the block, which the head's leaf owns, prints ahead of the head, and an indentable block
leading the value hangs it under the operator — prettier's own `chooseLayout` reading, one
shell deeper than the operator gap.

Authored (`unformatted_ours_inner_shell.svelte`):

```ts
y = (// c
/*
 */ a.b).m(1).n(2).o(3);
```

tsv (`input.svelte`), in one pass:

```ts
y =
	/*
	 */ a // c
		.b.m(1)
		.n(2)
		.o(3);
```

Prettier moves the `//` past `.b` from `input` (`output_prettier.svelte`). From the authored
form it prints `y = a// c⏎/*⏎ */ .b…` first, and on its second pass **merges** the `//` into
the block's text (`y = a/* // c⏎ */ .b`), so `// c` survives only as the block's content
(`audit_signature_inner_shell.txt` pins the chain).

Reason: content integrity. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell, indentable block under a `//`) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
