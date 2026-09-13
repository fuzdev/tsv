# colon_empty_tail_prettier_divergence

A value whose `:` has **nothing after it** — `f(a:)` inside a group, and `a:`, `:`, `a: b:`
at the top level of a custom property's value.

tsv: ends the value at the colon (`f(a:)`, `a:`), the way it drops the whitespace before
every `;`
Prettier: writes the space it gives every colon and leaves it stranded (`f(a: )`, `a: `),
holding that form on its own output

`◆design_choice`, in the frame's narrow sense: both formatters normalize every authoring of
this value to exactly one form — a glued `a:b:` and a spaced `a : b :` reach the same place
on each side — and the two pick a different representative. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Colon with an empty tail".

## Why prettier writes the space

`printCommaSeparatedValueGroup` prints a `value-colon` as the colon plus its separator and
then moves on to the next node; with no next node the separator is still written, so the
declaration's `;` lands after it. The same arm produces the run's other spaces, which is why
the space is there whether or not the author wrote one — and prettier re-reads the stranded
space as its own output's trailing whitespace and keeps it, so the form is stable rather than
oscillating.

## Why tsv drops it

Whitespace before a `;` carries no information and tsv normalizes it away everywhere else in
a declaration — including the same construct with a value after the colon, where the two
formatters agree ([colon](../colon/), [colon_custom_property](../colon_custom_property/)).
Writing a space *after* the last token of a value is the one case where prettier's own
separator rule outlives the thing it separates.

## Cells

| authoring | tsv | prettier |
| --- | --- | --- |
| `f(a:)` | `f(a:)` | `f(a: )` |
| `--b: a:` | `a:` | `a: ` |
| `--c: :` | `:` | `: ` |
| `--d: a: b:` | `a: b:` | `a: b: ` |

The first cell is on a **plain** property, where the colon is inside a group: the divergence
is the colon printer's, not the custom property's. The `unformatted_ours_*` variants carry
the glued (`a:b:`) and spaced (`a : b :`) authorings of the last cell, both of which tsv
normalizes to input and prettier does not.

## Related

- [colon](../colon/) — a `:` inside a function or a group with a value after it, where the
  two agree
- [colon_custom_property](../colon_custom_property/) — the same colon at a custom property's
  top level, also agreeing
- [important_trailing_whitespace](../../../declarations/important_trailing_whitespace_prettier_divergence/) —
  the other whitespace-before-`;` divergence, reached by a different printer (prettier copies
  postcss's raw `!important` tail there; here it fabricates the space)
