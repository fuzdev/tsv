# after_comma_block_stranded_prettier_divergence

A non-last declarator's block comment **stranded** after the comma — the author put
a newline between the comment and the next declarator (`a = 1, /* c */⏎ b = 2`). tsv
respects that newline and keeps the comment where it was written (trailing the comma
line); prettier attaches it to the preceding initializer and relocates it **before**
the comma.

```
// input (author's placement)   // tsv (preserve)        // prettier (relocate)
let a = 1, /* c */              let a = 1, /* c */        let a = 1 /* c */,
	b = 2;                          b = 2;                    b = 2;
```

This is the stranded counterpart of the hugging case: when the comment instead
**hugs** the next declarator (`a = 1, /* c */ b = 2`, no newline between them), tsv
leads the next declarator with it and both formatters agree — the single rule is
*a comment hugging the next declarator leads it; a stranded comment stays on the
comma line*.

The second statement combines a **before-comma** block with a **stranded**
after-comma block in the same gap (`a = 1 /* c1 */, /* c2 */⏎ b = 2`). Each stays
where the author wrote it — `/* c1 */` before the comma (trailing the initializer),
`/* c2 */` after it — while prettier relocates **both** before the comma
(`a = 1 /* c1 */ /* c2 */,`). The two halves compose independently.

The declarator-gap counterpart of the call-argument
[nonlast_arg_after_comma_block_stranded](../../../../expressions/calls/nonlast_arg_after_comma_block_stranded_prettier_divergence/).
The stranded block is a *stable* form only when the declarators sit on separate
lines: the multi-declarator statement always breaks them, while the shared for-init
gap collapses to one line when it fits (there the block hugs the next declarator and
both formatters agree) and diverges the same way only once the declarators wrap —
see [for-init](../../../../statements/for/init_after_comma_block_stranded_prettier_divergence/).
See
[conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §Comment relocation.
