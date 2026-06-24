# multiline_arg_nonlast_after_comma_block_then_line_prettier_divergence

The joined-args-path form of the after-comma block+line divergence. A call with
a multiline-content argument (here a multiline template) routes its arguments
through the shared joined-args path; a non-last argument with a block comment
after the comma plus a line comment (`a, /* c1 */ // c2`) keeps the block on the
comma line. Prettier relocates it before the comma.

```
// tsv                          // prettier
fn(                             fn(
	a, /* c1 */ // c2                 a /* c1 */, // c2
	`line1                            `line1
line2`                          line2`
);                              );
```

Reason: Comment relocation. See the plain-call sibling
([nonlast_arg_after_comma_block_then_line](../nonlast_arg_after_comma_block_then_line_prettier_divergence/))
for the full rationale, and
[conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
