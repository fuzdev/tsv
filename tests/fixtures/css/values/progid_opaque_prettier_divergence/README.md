# progid_opaque_prettier_divergence

A declaration value starting with `progid:` — the legacy IE filter syntax — is **opaque** on both
formatters: no number, quote, hex or unit normalization, whitespace and newlines kept as written,
never wrapped (the agreeing cells: [progid_opaque](../progid_opaque/)). Two cells differ.

**A comment inside the value.**
tsv: keeps it where the author wrote it (`progid:X.Y(a=1.50) /* c */ progid:Z.W(b=2)`), and a
trailing one too (`… /* c */;`)
Prettier: **drops** it, leaving the spaces on either side (`progid:X.Y(a=1.50)  progid:Z.W(b=2)`,
`progid:X.Y(a=1.50) ;` — a second pass then trims that trailing space, hence the pinned chain). Its
`progid:` arm returns a `value-unknown` node the AST walk discards, so the declaration keeps
postcss's own `value` string — and postcss's `raw()` strips a comment that has whitespace on
either side out of `value` (it survives only in `raws.value.raw`). A comment glued to its
neighbours on both sides (`)/* c */progid`) stays in postcss's `value`, so that cell agrees.

**A `progid:` that is not the value's first bytes.**
tsv: an ordinary glued token, kept whole (`alpha(opacity=50) progid:X.Y(a=1.50)`); the numbers
around it normalize as usual
Prettier: not the opaque arm (the trigger is `value.startsWith("progid:")`), so its value printer
reads the `:` as a separator and inserts a space after it (`progid: X.Y(a=1.50)`).

## Reason

Content preservation. A comment the author wrote is not the formatter's to delete, and the
opaque rule's whole point is to change nothing inside a value it does not understand — dropping
a comment is exactly such a change. The mid-value cell is the same rule read at a different
position: a token the formatter does not model is kept whole rather than split at a colon.
See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values)
("`progid:` opaque value").

## Related

- [progid_opaque](../progid_opaque/) — the agreeing cells: normalization, whitespace, the newline,
  the leading run, `!important`, the 129-column value, the quoted `-ms-filter` control
- [normalization_with_comment](../normalization_with_comment/) — the comment-bearing value that
  DOES normalize, the path a non-`progid:` value takes
