# media_value_newline_prettier_divergence

A `media-value` is a node prettier emits verbatim, so a whitespace run inside a feature
expression's value is the author's and both formatters keep it — `(a: x  ,  9px)` is a
fixed point on each. One spelling is tsv's exception: a run carrying a **newline**
collapses to a single space.

tsv: `@media (a: b c)`
Prettier: `@media (a: b⏎\tc)` — kept, and its own fixed point (`prettier_variant_newline`)

## Reason

Design choice. tsv never emits a raw newline inside an at-rule prelude: the prelude's own
line breaks are re-decided by the `and`/`or` fill against print width, and a newline
carried through from the source would be counted as prelude text by that fill — the
width accounting and the wrap would then disagree with the bytes on the page. Collapsing
is lossless in the only sense the run has (it is separator whitespace inside the value's
text, not content), and the collapsed form is a fixed point on both formatters.

The rest of the run rule is a **match**, pinned by
[media_value_verbatim_run](../media_value_verbatim_run/). See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
(`@media value newline`).
