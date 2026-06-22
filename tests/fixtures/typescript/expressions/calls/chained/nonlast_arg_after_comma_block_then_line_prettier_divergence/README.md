# nonlast_arg_after_comma_block_then_line_prettier_divergence (chained)

The member-callee-chain form of the after-comma block+line divergence: a
non-last argument of a chained call with a block comment after the comma plus a
line comment (`a, /* c1 */ // c2`). tsv keeps the block on the comma line, the
line comment trailing via `line_suffix`; Prettier relocates the block before the
comma.

```
// tsv                          // prettier
foo                             foo
	.bar(                           .bar(
		a, /* c1 */ // c2               a /* c1 */, // c2
		b                               b
	)                               )
	.baz();                         .baz();
```

See the plain-call sibling
([nonlast_arg_after_comma_block_then_line](../../nonlast_arg_after_comma_block_then_line_prettier_divergence/))
for the full rationale, and
[conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
