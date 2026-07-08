# param_after_comma_block_stranded_prettier_divergence

A non-first parameter's block comment **stranded** after the comma — the author put
a newline between the comment and the next param (`(a, /* c */⏎ b)`). tsv respects that
newline and keeps the comment where it was written (trailing the comma line); prettier
attaches it to the preceding param and relocates it **before** the comma.

```
// input (author's placement)   // tsv (preserve)       // prettier (relocate)
(                               (                        (
	a, /* c */                      a, /* c */               a /* c */,
	b                               b                        b
) => {}                         ) => {}                  ) => {}
```

This is the stranded counterpart of the hugging case: when the comment instead
**hugs** the next param (`(a, /* c */ b)`, no newline between them), tsv leads the next
param with it and both formatters agree — the single rule is *a comment hugging the
next param leads it; a stranded comment stays on the comma line*.

The stranded block is a *stable* form only when the params sit on separate lines. When
the param list **fits**, it collapses to one line — the stranded block hugs the next
param (`(a, /* c */ b)`) and both formatters agree — so the divergence surfaces only
once the params wrap (both cases here). The second case combines a **before-comma**
block with a **stranded** after-comma block in the same gap (`a /* c1 */, /* c2 */⏎ b`):
each stays on its own side of the comma while prettier relocates **both** before it
(`a /* c1 */ /* c2 */,`). The two halves compose independently.

The function-parameter counterpart of the call-argument
[nonlast_arg_after_comma_block_stranded](../../../expressions/calls/nonlast_arg_after_comma_block_stranded_prettier_divergence/)
and the variable-declarator
[after_comma_block_stranded](../../../declarations/variable/multiple/after_comma_block_stranded_prettier_divergence/).
One rule (`is_stranded_after_comma_block`) across every comma-separated site.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
