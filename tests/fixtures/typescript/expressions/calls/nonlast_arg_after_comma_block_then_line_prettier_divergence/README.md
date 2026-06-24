# nonlast_arg_after_comma_block_then_line_prettier_divergence

A non-last call argument with a block comment **after** the comma plus a line
comment after it (`a, /* c1 */ // c2`). tsv keeps the block on the comma line
where the author wrote it (the line comment trails via `line_suffix`); Prettier
relocates the block to **before** the comma.

```
// tsv                          // prettier
fn(                             fn(
	a, /* c1 */ // c2                 a /* c1 */, // c2
	b                                 b
);                              );
```

## Reason

tsv treats comment placement as intentional (see Comment Position Philosophy).
The author parked the block after the comma; moving it before the comma is a
syntactic-position change. tsv preserves it in place, idempotently.

This is the dual of the before-comma case (`a /* c1 */, // c2`), where both
formatters already agree and keep the block before the comma
([nonlast_arg_block_then_line_comment](../nonlast_arg_block_then_line_comment/)).
Prettier canonicalizes both authored positions to before-comma; tsv preserves
whichever the author chose. The same after-comma preservation applies across
every argument path — `new`
([new_nonlast_arg_after_comma_block_then_line](../new_nonlast_arg_after_comma_block_then_line_prettier_divergence/)),
the joined-args path
([multiline_arg_nonlast_after_comma_block_then_line](../multiline_arg_nonlast_after_comma_block_then_line_prettier_divergence/)),
and member-callee chains
([chained/nonlast_arg_after_comma_block_then_line](../chained/nonlast_arg_after_comma_block_then_line_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
