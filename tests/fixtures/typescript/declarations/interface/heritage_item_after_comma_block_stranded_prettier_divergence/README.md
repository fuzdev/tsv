# heritage_item_after_comma_block_stranded_prettier_divergence

A non-last `extends` item's block comment **stranded** after the comma — the author
put a newline between the comment and the next item (`A, /* c */⏎ B`). tsv respects
that newline and keeps the comment where it was written (trailing the comma line);
prettier attaches it to the preceding item and relocates it **before** the comma.

```
// input (author's placement)   // tsv (preserve)       // prettier (relocate)
extends                         extends                  extends
	A, /* c */                      A, /* c */               A /* c */,
	B                               B                        B
```

The heritage counterpart of the variable-declarator
[after_comma_block_stranded](../../variable/multiple/after_comma_block_stranded_prettier_divergence/)
and for-init
[init_after_comma_block_stranded](../../../statements/for/init_after_comma_block_stranded_prettier_divergence/)
— all share `is_stranded_after_comma_block`, so a heritage clause preserves the
author's comma-side placement the same way those declarator gaps do.

This is the stranded counterpart of the hugging case: when the comment instead
**hugs** the next item (`A, /* c */ B`, no newline between them), tsv leads the next
item with it and both formatters agree — see the plain-match sibling
[heritage_item_after_comma_block](../heritage_item_after_comma_block/). The single
rule: *a block hugging the next item leads it; a stranded block stays on the comma
line*. The deliberately long identifiers force the items to wrap, which is what makes
the stranded block a stable divergence (an inline clause collapses the newline and
both formatters agree).

See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
