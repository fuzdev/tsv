# init_after_comma_block_stranded_prettier_divergence

A non-last for-init declarator's block comment **stranded** after the comma — the
author put a newline between the comment and the next declarator (`a = 1, /* c */⏎ b`).
tsv respects that newline and keeps the comment where it was written (trailing the
comma line); prettier attaches it to the preceding initializer and relocates it
**before** the comma.

```
// input (author's placement)   // tsv (preserve)        // prettier (relocate)
let a = 1, /* c */              let a = 1, /* c */        let a = 1 /* c */,
	b = 2;                          b = 2;                    b = 2;
```

The for-init counterpart of the variable-declarator
[after_comma_block_stranded](../../../declarations/variable/multiple/after_comma_block_stranded_prettier_divergence/)
and the call-argument
[nonlast_arg_after_comma_block_stranded](../../../expressions/calls/nonlast_arg_after_comma_block_stranded_prettier_divergence/)
— all three share `is_stranded_after_comma_block`. The stranded block is a stable
form only when the declarators sit on separate lines: a multi-declarator statement
always breaks them, but the for-init gap collapses to one line when it fits (there
the block hugs the next declarator and both formatters agree). The deliberately long
identifiers here force the declarators to wrap, making the stranded block a stable
divergence.

See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
