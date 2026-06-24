# new_nonlast_arg_after_comma_block_then_line_prettier_divergence

The `new`-argument form of the after-comma block+line divergence: a non-last
`new` argument with a block comment after the comma plus a line comment
(`a, /* c1 */ // c2`). tsv keeps the block on the comma line; Prettier relocates
it before the comma.

```
// tsv                          // prettier
new A(                          new A(
	a, /* c1 */ // c2                 a /* c1 */, // c2
	b                                 b
);                              );
```

The blank-line case routes through the blank-line args path, which must
preserve the after-comma position too.

Reason: Comment relocation. See the plain-call sibling
([nonlast_arg_after_comma_block_then_line](../nonlast_arg_after_comma_block_then_line_prettier_divergence/))
for the full rationale, and
[conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
