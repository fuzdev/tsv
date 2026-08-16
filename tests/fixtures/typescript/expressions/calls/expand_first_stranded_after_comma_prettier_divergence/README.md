# expand_first_stranded_after_comma_prettier_divergence

A block comment **stranded** after the FIRST argument's comma (`() => {…}, /* c */⏎b`).
It trails that argument, which is what declines the expand-first hug — prettier's
`shouldExpandFirstArg` opens with `!hasComment(firstArg)`, and prettier binds a
same-line-after comment as the preceding argument's trailing comment. Both formatters
therefore break every argument out; they differ only in which side of the comma the
block lands on.

```
// tsv                          // prettier
fn(                             fn(
	() => {                         () => {
		a();                            a();
	}, /* c */                      } /* c */,
	b                               b
);                              );
```

## Reason

tsv treats comment placement as intentional (see Comment Position Philosophy). The author
parked the block after the comma; moving it before the comma is a syntactic-position
change. tsv preserves it in place, idempotently — the same rule the plain non-last gap
follows
([nonlast_arg_after_comma_block_stranded](../nonlast_arg_after_comma_block_stranded_prettier_divergence/)),
here reached through the expand-first refusal rather than the ordinary argument list.

The last example pins the **before-comma** spelling of the same refusal, where the two
formatters agree on the block's position — so the divergence is the comma side alone, not
the refusal.

Covers the plain-call, `new`, and chained-call argument paths. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
