# nonlast_arg_after_comma_block_stranded_prettier_divergence

A non-last argument's block comment **stranded** after the comma — the author put
a newline between the comment and the next argument (`a, /* c */⏎ b`). tsv respects
that newline and keeps the comment where it was written (trailing the comma line);
prettier attaches it to the preceding argument and relocates it **before** the comma.

```
// input (author's placement)   // tsv (preserve)        // prettier (relocate)
fn(                             fn(                       fn(
	a, /* c */                      a, /* c */                a /* c */,
	b                               b                         b
);                              );                        );
```

This is the stranded counterpart of the hugging case: when the comment instead
**hugs** the next argument (`a, /* c */ b`, no newline between them), tsv leads the
next argument with it (`C`) and both formatters agree — see the plain-match
siblings ([new](../new_nonlast_arg_after_comma_block/),
[plain](../nonlast_arg_after_comma_block/),
[chain](../chained/nonlast_arg_after_comma_block/)). The single rule across all
argument paths: *a comment hugging the next arg leads it; a stranded comment stays
on the comma line.*

The second `fn` example combines a **before-comma** block with a **stranded**
after-comma block in the same gap (`a /* b1 */, /* s */⏎ b`). Each stays where the
author wrote it — `/* b1 */` before the comma (trailing the arg), `/* s */` after it
— while prettier relocates **both** before the comma (`a /* b1 */ /* s */,`). The
two halves compose: the before-comma rule and the stranded rule hold independently.

Covers the plain-call, `new`, and chained-call argument paths. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
