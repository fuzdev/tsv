# important_trailing_whitespace_prettier_divergence

Whitespace between a declaration's `!important` and its `;` — `color: red !important   ;`, with or
without a comment after the keyword.

tsv: normalizes it away (`color: red !important;`), the way it drops the whitespace before every
other `;`
Prettier: keeps it verbatim, as a stable form of its own (`prettier_variant_spaces`). Its
declaration printer emits `node.raws.important` — postcss's raw spelling of the tail, which
runs from the whitespace before `!` to the `;` — with only the `\s*!\s*important` core rewritten
to ` !important`, so what follows the keyword is copied.

## Reason

Whitespace normalization: the gap carries no information, and prettier itself normalizes the
same gap on a declaration without `!important` (`color: red   ;` → `color: red;`). The raw copy
is an accident of printing `raws.important` rather than the keyword. Not a `progid:` cell — found
by that class's probe, but every declaration takes it.
See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values)
("`!important` trailing whitespace").

## Related

- [empty_value_important](../../values/variables/empty_value_important_prettier_divergence/) — the
  other `!important` spacing quirk, on the value side of the keyword
