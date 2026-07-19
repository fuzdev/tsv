# fill_tail_wide_char_long_prettier_divergence

The parity-shifted tail case with a **wide (CJK) tail word**, pinning that the fill's break
decision measures **visual width**, not bytes or chars.

`日本` is two glyphs but **four columns**. The first `<li>` lands at exactly 100 and stays
inline; the second is one pad character longer, so it reaches 101 and the tail breaks to its
own line. A byte-based measure would read the second line as 105 and break too early; a
char-based one would read 99 and wrongly keep it inline. Only visual width gives 101.

```
tsv       `…aaaa ~{xxxxxxxxxx}` / `日本`     101 → breaks
Prettier  `…aaaa ~{xxxxxxxxxx} 日本`         keeps at 101
```

This guards a class the corpus gates are structurally blind to: a width error only changes
output when it crosses the print width, so a wrong multiplier on a rare character leaves every
formatted file byte-identical. The 100/101 pair is the whole test — it must fail if the wide
glyph is ever counted as anything but 2 columns.

`prettier_variant_joined` holds the joined authoring: prettier keeps it stable at 101, tsv
rewrites it to `input`. So the divergence is one of normalization.

## Reason

Print width is a hard limit in tsv. See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
The plain-ASCII form of this case is
[fill_tail_after_expr_long](../fill_tail_after_expr_long_prettier_divergence/).
