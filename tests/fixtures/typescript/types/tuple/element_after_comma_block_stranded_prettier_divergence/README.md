# element_after_comma_block_stranded_prettier_divergence

A tuple element's block comment **stranded** after the comma — the author left a newline
before the next element (`[a, /* c */⏎ b]`). tsv respects that newline and keeps the
comment where it was written (trailing the comma line); prettier attaches it to the
preceding element and relocates it **before** the comma.

```
// input (author's placement)   // tsv (preserve)       // prettier (relocate)
type A = [                      type A = [               type A = [
	a, /* c */                      a, /* c */               a /* c */,
	b                               b                        b
];                              ];                       ];
```

The `B` case pairs a **before-comma** block with a stranded after-comma block in the same
gap (`a /* c1 */, /* c2 */⏎ b`): each stays on its own side of the comma while prettier
relocates **both** before it, merging them onto one trailing run.

A block **hugging** the next element (`a, /* c */ b`, no newline between them) leads that
element and both formatters agree — [element_after_comma_block](../element_after_comma_block/).
The stranded form is stable only once the elements sit on separate lines; a tuple that fits
collapses inline, where the block hugs and both formatters agree again. The comma pushed
onto its own line with the comment (`a⏎, /* c */⏎ b`) is the same authoring one notch
further — the comma is re-emitted structure, outside every element span, so the comment
still sits after it — and takes the same normalization
(`unformatted_ours_comma_own_line`).

The tuple member of the `is_stranded_after_comma_block` family — see the
[type-parameter](../../type_params/param_after_comma_block_stranded_prettier_divergence/),
[function/constructor-type](../../function_type/param_after_comma_block_stranded_prettier_divergence/)
and value-level
[declarator](../../../declarations/variable/multiple/after_comma_block_stranded_prettier_divergence/)
siblings.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
