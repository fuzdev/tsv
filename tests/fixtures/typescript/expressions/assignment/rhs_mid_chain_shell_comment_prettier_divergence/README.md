# Value's mid-chain shell comment

A comment inside grouping parens around a member chain's base below its head, at an
assignment's value, a declarator's initializer and a class field (`x = (// a⏎x.y).q()`). The
chain strips the shell and prints its run in the member gap after the head, where the
paren-free spelling (`x = x // a⏎.y.q()`) carries it.

- **tsv**: one pass, the run kept after the head, the value laid out as the paren-free
  spelling is (`unformatted_ours_stripped_shell.svelte` normalizes to `input.svelte`):

```ts
x = x // a
	.y.q();
```

  The layout reads the run where it prints: a `//` there keeps the value on the operator's
  line, and a comment on the head's line makes the chain no poorly breakable one, so the
  value never hangs under the `=` for a comment the reparse reads in the member gap.

- **prettier**: moves each `//` past the next member (`x = x.y // a⏎\t.q();`), and a
  `[0]()` tail's to the statement end (`k = x.y[0](); // a`). From the shell spelling its
  first pass glues the comment to the head (`x// a`, `audit_signature_stripped_shell.txt`);
  from `input.svelte` the own-line cells take three passes and the two-comment cell is
  **reordered** (`x = x /* b */.y // a`, the block now ahead of the `//`), pinned in
  `audit_signature.txt`.

A block-only run reaches prettier's fixed point
([rhs_mid_chain_shell_block_comment_long](../rhs_mid_chain_shell_block_comment_long_prettier_divergence/)).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
